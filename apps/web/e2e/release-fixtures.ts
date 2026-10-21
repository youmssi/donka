import type { APIRequestContext } from '@playwright/test';
import type { ApiSchemas } from '../src/components/shared/api';
import { expect, unique } from './fixtures';

/** Explicit API fixtures; browser journeys exercise the actions they claim separately. */
export async function ruleProject(admin: APIRequestContext, prefix: string) {
  const key = unique(prefix);
  const response = await admin.post('/api/v1/projects', {
    data: { key, name: 'Release verification', description: '' },
  });
  await expect(response).toBeOK();
  const project: ApiSchemas['ProjectResponse'] = await response.json();
  return { ...project, path: `/api/v1/projects/${project.id}` };
}

export function expressionGraph(factor: number) {
  return {
    nodes: [
      { id: 'input', name: 'Request', type: 'inputNode', position: { x: 0, y: 0 } },
      {
        id: 'calculate',
        name: 'Calculate',
        type: 'expressionNode',
        position: { x: 0, y: 160 },
        content: { expressions: [{ id: 'value', key: 'output', value: `input * ${factor}` }] },
      },
      { id: 'output', name: 'Response', type: 'outputNode', position: { x: 0, y: 320 } },
    ],
    edges: [
      { id: 'in', sourceId: 'input', targetId: 'calculate' },
      { id: 'out', sourceId: 'calculate', targetId: 'output' },
    ],
  };
}

export async function snapshot(admin: APIRequestContext, path: string, notes: string) {
  const response = await admin.post(`${path}/releases`, { data: { bump: 'minor', notes } });
  await expect(response).toBeOK();
  const release: ApiSchemas['ReleaseResponse'] = await response.json();
  return release;
}

export async function version(admin: APIRequestContext, path: string, message: string) {
  const current = await admin.get(path);
  await expect(current).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await current.json();
  await expect(await admin.post(`${path}/versions`, { data: { message, revision: decision.revision } })).toBeOK();
}

export async function waitLive(
  admin: APIRequestContext,
  path: string,
  environment: 'staging' | 'production',
  releaseId: string,
) {
  await expect
    .poll(
      async () => {
        const response = await admin.get(`${path}/environments`);
        await expect(response).toBeOK();
        const body: ApiSchemas['EnvironmentListResponse'] = await response.json();
        return body.items.find((item) => item.environment === environment)?.live?.releaseId;
      },
      { timeout: 60_000 },
    )
    .toBe(releaseId);
}

export async function stage(admin: APIRequestContext, path: string, releaseId: string) {
  await expect(await admin.post(`${path}/environments/staging/deployments`, { data: { releaseId } })).toBeOK();
  await waitLive(admin, path, 'staging', releaseId);
}

export async function issueToken(
  admin: APIRequestContext,
  path: string,
  environment: 'staging' | 'production',
  name: string,
) {
  const response = await admin.post(`${path}/environments/${environment}/tokens`, { data: { name } });
  await expect(response).toBeOK();
  const token: ApiSchemas['IssuedTokenResponse'] = await response.json();
  return token;
}

export function connectorGraph(config: Record<string, unknown>, inputSchema?: Record<string, unknown>) {
  return {
    nodes: [
      {
        id: 'input',
        name: 'Request',
        type: 'inputNode',
        position: { x: 0, y: 0 },
        ...(inputSchema ? { content: { schema: JSON.stringify(inputSchema) } } : {}),
      },
      {
        id: 'bureau',
        name: 'Bureau',
        type: 'customNode',
        position: { x: 0, y: 160 },
        content: { kind: 'donka.connector', config },
      },
      { id: 'output', name: 'Response', type: 'outputNode', position: { x: 0, y: 320 } },
    ],
    edges: [
      { id: 'in', sourceId: 'input', targetId: 'bureau' },
      { id: 'out', sourceId: 'bureau', targetId: 'output' },
    ],
  };
}
