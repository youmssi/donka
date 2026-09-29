'use client';

// The decision editor (antd + WASM): loaded only on the editor page, never on the server.
import '@ant-design/v5-patch-for-react-19';
import './monaco-setup';
import '@gorules/jdm-editor/dist/style.css';

import { DecisionGraph, GraphSimulator, JdmConfigProvider, type Simulation } from '@gorules/jdm-editor';
import { PlayCircle } from 'lucide-react';
import { useTheme } from 'next-themes';
import { useMemo, useState } from 'react';

import type { ActionResult } from '@/components/shared/api';

import { decisionNodeSpecification, type DecisionNodeLabels } from './decision-node';
import type { SimulationResult } from './schema';

type Graph = Parameters<NonNullable<Parameters<typeof DecisionGraph>[0]['onChange']>>[0];

export interface JdmGraphProps {
  value: unknown;
  onChange: (graph: unknown) => void;
  disabled: boolean;
  simulatorTitle: string;
  /** Decisions this one may call (the project's others). */
  callable: string[];
  decisionNodeLabels: DecisionNodeLabels;
  /** Runs the graph with an input; the page decides where (the project's simulator). */
  simulate: (graph: unknown, context: unknown) => Promise<ActionResult<SimulationResult>>;
  /** Text for a failed run, in the reader's language. */
  failureMessage: (result: Extract<ActionResult<SimulationResult>, { ok: false }>) => string;
}

/**
 * Studio's colours, read from its design tokens, as the editor's theme needs them.
 * This component only renders in the browser, so the tokens are read while
 * rendering, once per light or dark mode.
 */
function useEditorTheme() {
  const { resolvedTheme } = useTheme();
  const mode = resolvedTheme === 'dark' ? ('dark' as const) : ('light' as const);
  const token = useMemo(
    () => ({
      colorPrimary: tokenColor('--primary'),
      colorBgContainer: tokenColor('--background'),
      colorBgElevated: tokenColor('--popover'),
      colorBorder: tokenColor('--border'),
      colorText: tokenColor('--foreground'),
      colorTextSecondary: tokenColor('--muted-foreground'),
      fontFamily: getComputedStyle(document.body).fontFamily,
    }),
    // The tokens change with the mode.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [mode],
  );
  return { mode, token };
}

/**
 * A CSS colour token as `#rrggbb`. Studio's tokens are `oklch()`, which the
 * editor's theme cannot derive shades from; the browser converts it by painting
 * one pixel.
 */
function tokenColor(name: string): string {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 1;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) return '';
  context.fillStyle = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  context.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0] = context.getImageData(0, 0, 1, 1).data;
  return `#${[r, g, b].map((part) => part.toString(16).padStart(2, '0')).join('')}`;
}

export function JdmGraph({
  value,
  onChange,
  disabled,
  simulatorTitle,
  callable,
  decisionNodeLabels,
  simulate,
  failureMessage,
}: JdmGraphProps) {
  const theme = useEditorTheme();
  const [simulation, setSimulation] = useState<Simulation>();
  const components = useMemo(
    () => [decisionNodeSpecification(callable, decisionNodeLabels)],
    [callable, decisionNodeLabels],
  );
  const [running, setRunning] = useState(false);

  async function run({ graph, context }: { graph: Graph; context: unknown }) {
    setRunning(true);
    const result = await simulate(graph, context);
    setRunning(false);
    if (result.ok) {
      setSimulation({
        result: {
          result: result.data.result as never,
          trace: result.data.trace as never,
          performance: result.data.performance,
          snapshot: graph,
        },
      });
      return;
    }
    // The engine's error names the node that failed and carries the trace up to it.
    const details = (result.error.details ?? {}) as { nodeId?: string; source?: string; trace?: unknown };
    setSimulation({
      result: details.trace
        ? { result: null as never, trace: details.trace as never, performance: '', snapshot: graph }
        : undefined,
      error: {
        title: failureMessage(result),
        message: details.source ?? '',
        data: { nodeId: details.nodeId },
      },
    });
  }

  return (
    <JdmConfigProvider theme={theme}>
      <DecisionGraph
        value={value as Graph}
        onChange={onChange}
        disabled={disabled}
        simulate={simulation}
        reactFlowProOptions={{ hideAttribution: true }}
        components={components}
        panels={[
          {
            id: 'simulator',
            title: simulatorTitle,
            icon: <PlayCircle size={16} aria-hidden />,
            renderPanel: () => (
              <GraphSimulator loading={running} onClear={() => setSimulation(undefined)} onRun={(p) => void run(p)} />
            ),
          },
        ]}
      />
    </JdmConfigProvider>
  );
}
