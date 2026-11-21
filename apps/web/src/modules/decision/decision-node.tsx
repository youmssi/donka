'use client';

import { GraphNode, useDecisionGraphActions, useDecisionGraphState, type NodeSpecification } from '@gorules/jdm-editor';
import { Workflow } from 'lucide-react';

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

interface DecisionNodeContent {
  key?: string;
}

export interface DecisionNodeLabels {
  displayName: string;
  shortDescription: string;
  choose: string;
}

/**
 * The node that calls another decision of the project (`decisionNode`). The
 * editor has no built-in one: it is given through its `components` extension
 * point, with the project's decisions to choose from.
 */
export function decisionNodeSpecification(
  keys: string[],
  labels: DecisionNodeLabels,
): NodeSpecification<DecisionNodeContent> {
  return {
    type: 'decisionNode',
    displayName: labels.displayName,
    shortDescription: labels.shortDescription,
    icon: <Workflow size={16} aria-hidden />,
    generateNode: ({ index }) => ({ name: `decision${index}`, content: {} }),
    renderNode: ({ specification, id, selected, data }) => (
      <GraphNode id={id} specification={specification} name={data.name} isSelected={selected}>
        <ChooseDecision id={id} keys={keys} labels={labels} />
      </GraphNode>
    ),
  };
}

function ChooseDecision({ id, keys, labels }: { id: string; keys: string[]; labels: DecisionNodeLabels }) {
  const { updateNode } = useDecisionGraphActions();
  const disabled = useDecisionGraphState(({ disabled }) => disabled);
  // A node's content lives in the graph's store; the canvas only hands renderers its name.
  const value = useDecisionGraphState(
    ({ decisionGraph }) =>
      (decisionGraph.nodes.find((node) => node.id === id)?.content as DecisionNodeContent | undefined)?.key,
  );
  // A key that is no longer in the project still shows, so the problem is visible.
  const options = value && !keys.includes(value) ? [value, ...keys] : keys;
  return (
    <Select
      value={value}
      disabled={disabled}
      onValueChange={(key) =>
        updateNode(id, (draft) => {
          draft.content = { ...(draft.content as DecisionNodeContent), key };
          return draft;
        })
      }
    >
      <SelectTrigger size="sm" className="nodrag w-full font-mono text-xs" aria-label={labels.choose}>
        <SelectValue placeholder={labels.choose} />
      </SelectTrigger>
      <SelectContent>
        {options.map((key) => (
          <SelectItem key={key} value={key} className="font-mono text-xs">
            {key}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
