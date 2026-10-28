import type { APIRequestContext } from '@playwright/test';
import type { ApiSchemas } from '../src/components/shared/api';
import { expect, passwordLink, unique, type UiOptions } from './fixtures';
import {
  connectorGraph as graphWithConnector,
  expressionGraph,
  issueToken,
  ruleProject,
  snapshot,
  stage,
  version,
} from './release-fixtures';
import { origin, settings } from './settings';

function connectorGraph(url: string, fail: boolean) {
  const schema = {
    type: 'object',
    properties: {
      caseId: { type: 'string' },
      applicant: {
        type: 'object',
        properties: {
          nationalId: { type: 'string', 'x-donka': { identifier: true } },
          name: { type: 'string' },
        },
      },
    },
  };
  return graphWithConnector(
    {
      preset: 'bureau-score',
      url,
      auth: { type: 'header', header: 'X-Api-Key', secret: 'E2E_BUREAU_KEY' },
      body: { nationalId: 'CM-1' },
      outputKey: 'bureau',
      timeoutMs: 500,
      retries: 0,
      onError: fail ? 'fail' : 'fallback',
      fallback: { score: null, available: false },
      mock: { score: 1, available: true },
    },
    schema,
  );
}

/** Graphs/releases/tokens are API fixtures; every record is produced by the real Runtime feed. */
export async function recordProject(admin: APIRequestContext, services: APIRequestContext) {
  const project = await ruleProject(admin, 'records');
  const { key, path } = project;
  const connector = origin('DONKA_E2E_CONNECTOR_URL');
  const decisions = [];
  for (const [name, content] of [
    ['score', connectorGraph(`${connector}/${key}-record/score`, false)],
    ['failed', connectorGraph(`${connector}/${key}-failed/failure`, true)],
    ['plain', expressionGraph(2)],
  ] as const) {
    const added = await admin.post(`${path}/decisions`, { data: { key: name, content } });
    await expect(added).toBeOK();
    const decision: ApiSchemas['DecisionResponse'] = await added.json();
    decisions.push(decision);
    await version(admin, `${path}/decisions/${decision.id}`, 'Recorded decision fixture');
  }
  const [decision] = decisions;
  if (!decision) throw new Error('The recorded decision fixture is missing');
  await expect(
    await admin.put(`${path}/decision-log/settings`, {
      data: { outcomeField: 'bureau.score', redactedFields: [] },
    }),
  ).toBeOK();
  const release = await snapshot(admin, path, 'Recorded live service decisions');
  const token = await issueToken(admin, path, 'staging', 'Recorded decision caller');
  await stage(admin, path, release.id);
  const url = `${settings().runtime}/api/projects/${key}/evaluate`;
  await expect
    .poll(
      async () =>
        (
          await services.post(`${url}/score`, {
            headers: { 'X-Access-Token': 'not-issued' },
            data: { context: {} },
          })
        ).status(),
      { timeout: 60_000 },
    )
    .toBe(401);
  const input = {
    caseId: key,
    applicant: { nationalId: `SYNTHETIC-PRIVATE-ID-${key}`, name: `SYNTHETIC-PRIVATE-NAME-${key}` },
    note: 'Synthetic context '.repeat(24),
  };
  async function emit(decision = 'score', reference = unique('record'), context: unknown = input) {
    const response = await services.post(`${url}/${decision}`, {
      headers: { 'X-Access-Token': token.token, 'X-Donka-Reference': reference },
      data: { context },
    });
    expect(response.status()).toBe(decision === 'failed' ? 400 : 200);
    const id = response.headers()['x-decision-id'];
    expect(id, 'The configured Runtime names the record it queued').toMatch(/^[0-9a-f-]{36}$/);
    if (!id) throw new Error('Runtime decision id is missing');
    const body = await response.json();
    expect(body).not.toHaveProperty('trace');
    return { id, reference, body };
  }
  async function received(reference: string) {
    await expect
      .poll(
        async () => {
          const response = await admin.get(`${path}/decision-log`, { params: { reference } });
          await expect(response).toBeOK();
          return (await response.json()).total;
        },
        { timeout: 60_000 },
      )
      .toBe(1);
  }
  async function calls(decision: 'score' | 'failed' = 'score') {
    const response = await services.get(
      `${origin('DONKA_E2E_BUREAU_URL')}/calls/${key}-${decision === 'failed' ? 'failed' : 'record'}`,
    );
    await expect(response).toBeOK();
    return (await response.json()).calls;
  }
  return {
    ...project,
    release,
    input,
    decision,
    emit,
    received,
    calls,
    cleanup: async () => expect(await admin.delete(`${path}/environments/staging/tokens/${token.id}`)).toBeOK(),
  };
}

export async function recordViewer(
  admin: APIRequestContext,
  services: APIRequestContext,
  path: string,
  locale: UiOptions['appLocale'],
) {
  const email = `${unique('record-viewer')}@donka.local`;
  const password = 'disposable record viewer passphrase';
  await expect(await admin.post('/api/v1/users/invitations', { data: { email, locale, isAdmin: false } })).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', {
      data: { token: await passwordLink(services, email), password },
    }),
  ).toBeOK();
  const response = await admin.post(`${path}/members`, { data: { email, role: 'viewer' } });
  await expect(response).toBeOK();
  const member: ApiSchemas['MemberResponse'] = await response.json();
  return { ...member, password };
}
