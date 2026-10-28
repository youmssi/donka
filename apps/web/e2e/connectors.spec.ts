import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect } from './fixtures';
import { openPage } from './navigation';
import { saveVersion } from './decision-actions';
import { connectorGraph, issueToken, ruleProject, snapshot, stage, version } from './release-fixtures';
import { origin, settings } from './settings';

test('connector settings persist; Studio uses mocks while Runtime maps authenticated calls and bounds failures', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  await expect(await admin.put('/api/v1/me/tours/editor')).toBeOK();
  const project = await ruleProject(admin, 'connectors');
  const { path, key } = project;
  const bureau = origin('DONKA_E2E_BUREAU_URL');
  const connector = origin('DONKA_E2E_CONNECTOR_URL');
  const added = await admin.post(`${path}/decisions`, {
    data: { key: 'score', content: connectorGraph({ preset: 'bureau-score' }) },
  });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  const decisionPath = `${path}/decisions/${decision.id}`;
  let page = await openPage(context, `/${appLocale}/projects/decision?p=${key}&d=score`);
  await page.getByRole('button', { name: m.connector.settings, exact: true }).click();
  let sheet = page.getByRole('dialog', { name: m.connector.title, exact: true });
  await sheet.getByLabel(m.connector.url, { exact: true }).fill(`${connector}/${key}-score/score`);
  await sheet.getByLabel(m.connector.secret, { exact: true }).fill('invalid secret');
  await sheet.getByRole('button', { name: m.connector.apply, exact: true }).click();
  await expect(sheet.getByText(m.validation.secretName, { exact: true })).toBeVisible();
  await sheet.getByLabel(m.connector.secret, { exact: true }).fill('E2E_BUREAU_KEY');
  await sheet
    .getByLabel(m.connector.body, { exact: true })
    .fill(JSON.stringify({ nationalId: '{{ applicant.nationalId }}', nested: { region: '{{ region }}' } }));
  await sheet.getByLabel(m.connector.outputKey, { exact: true }).fill('bureau');
  await sheet.getByLabel(m.connector.timeout, { exact: true }).fill('500');
  await sheet.getByLabel(m.connector.retries, { exact: true }).fill('1');
  await sheet.getByLabel(m.connector.mock, { exact: true }).fill('{"score":1,"available":true}');
  await sheet.getByRole('button', { name: m.connector.apply, exact: true }).click();
  await expect(sheet).toBeHidden();
  let config: Record<string, unknown> = {};
  await expect
    .poll(async () => {
      const response = await admin.get(decisionPath);
      await expect(response).toBeOK();
      const body = await response.json();
      config = body.content.nodes.find((node: { id: string }) => node.id === 'bureau').content.config;
      return config.auth;
    })
    .toEqual({ type: 'header', header: 'X-Api-Key', secret: 'E2E_BUREAU_KEY' });
  expect(config).toMatchObject({
    body: { nationalId: '{{ applicant.nationalId }}', nested: { region: '{{ region }}' } },
    outputKey: 'bureau',
    timeoutMs: 500,
    retries: 1,
    onError: 'fallback',
    fallback: { score: null, available: false },
    mock: { score: 1, available: true },
  });
  page = await openPage(context, page.url(), page);
  await page.getByRole('button', { name: m.connector.settings, exact: true }).click();
  sheet = page.getByRole('dialog', { name: m.connector.title, exact: true });
  await expect(sheet.getByLabel(m.connector.secret, { exact: true })).toHaveValue('E2E_BUREAU_KEY');
  await expect(sheet.getByLabel(m.connector.url, { exact: true })).toHaveValue(`${connector}/${key}-score/score`);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await testInfo.attach('connector-settings', {
    body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
    contentType: 'image/png',
  });
  await sheet.getByRole('button', { name: m.common.cancel, exact: true }).click();
  const calls = async (name: string) => {
    const response = await services.get(`${bureau}/calls/${key}-${name}`);
    await expect(response).toBeOK();
    return (await response.json()).calls;
  };
  expect(await calls('score')).toEqual([]);
  await page.locator('.grl-dg__aside__side-bar__bottom button').click();
  const input = page.locator('.grl-dg__simulator textarea').first();
  await expect(input).toBeEditable();
  await input.focus();
  await input.press('ControlOrMeta+A');
  await page.keyboard.insertText('{"applicant":{"nationalId":"CM-1"},"region":"CM"}');
  const simulated = page.waitForResponse((reply) => reply.url().endsWith(`${decisionPath}/simulate`));
  await page.getByRole('button', { name: 'Run' }).click();
  const simulation = await simulated;
  expect(simulation.ok()).toBe(true);
  expect(await simulation.json()).toMatchObject({
    result: { bureau: { score: 1, available: true } },
    trace: { bureau: { traceData: { mode: 'mock', outcome: 'ok' } } },
  });
  expect(await calls('score')).toEqual([]);
  await saveVersion(page, decisionPath, 'UI-configured connector', m.versions);
  // Each additional graph is an explicit fixture for a distinct Runtime failure policy.
  const cases = [
    { name: 'retry', mode: 'retry', config: { retries: 1 }, outcome: 'ok', attempts: 2, score: 712 },
    {
      name: 'fallback',
      mode: 'failure',
      config: { retries: 1 },
      outcome: 'fallback',
      attempts: 2,
      error: 'status',
      score: null,
    },
    {
      name: 'timeout',
      mode: 'timeout',
      config: { timeoutMs: 100, retries: 0 },
      outcome: 'fallback',
      attempts: 1,
      error: 'timeout',
      score: null,
    },
    {
      name: 'non-json',
      mode: 'non-json',
      config: { retries: 1 },
      outcome: 'fallback',
      attempts: 1,
      error: 'not_json',
      score: null,
    },
    {
      name: 'missing',
      mode: 'score',
      config: { auth: { type: 'header', header: 'X-Api-Key', secret: 'E2E_UNCONFIGURED' } },
      outcome: 'fallback',
      attempts: 0,
      error: 'missing_secret',
      score: null,
    },
    {
      name: 'stop',
      mode: 'failure',
      config: { retries: 0, onError: 'fail' },
      outcome: 'error',
      attempts: 1,
      error: 'status',
      score: null,
    },
  ];
  for (const item of cases) {
    const response = await admin.post(`${path}/decisions`, {
      data: {
        key: item.name,
        content: connectorGraph({ ...config, url: `${connector}/${key}-${item.name}/${item.mode}`, ...item.config }),
      },
    });
    await expect(response).toBeOK();
    const body: ApiSchemas['DecisionResponse'] = await response.json();
    await version(admin, `${path}/decisions/${body.id}`, `Controlled ${item.name}`);
  }
  const breaker = await admin.post(`${path}/decisions`, {
    data: {
      key: 'breaker',
      content: connectorGraph({ ...config, url: `${connector}/${key}-breaker/failure`, retries: 0 }),
    },
  });
  await expect(breaker).toBeOK();
  const breakerDecision: ApiSchemas['DecisionResponse'] = await breaker.json();
  await version(admin, `${path}/decisions/${breakerDecision.id}`, 'Controlled circuit breaker');
  const release = await snapshot(admin, path, 'Controlled live connectors');
  const token = await issueToken(admin, path, 'staging', 'Connector verification');
  // Include the token in the first artifact; readiness must not race a later token republish.
  await stage(admin, path, release.id);
  const evaluate = (name: string, accessToken = token.token) =>
    services.post(`${settings().runtime}/api/projects/${key}/evaluate/${name}`, {
      headers: { 'X-Access-Token': accessToken },
      data: { context: { applicant: { nationalId: 'CM-1' }, region: 'CM' }, trace: true },
    });
  try {
    await expect.poll(async () => (await evaluate('score', 'not-issued')).status(), { timeout: 60_000 }).toBe(401);
    const response = await evaluate('score');
    await expect(response).toBeOK();
    expect(await response.json()).toMatchObject({
      result: { bureau: { score: 712, available: true } },
      trace: { bureau: { traceData: { mode: 'live', outcome: 'ok', attempts: 1, status: 200 } } },
    });
    expect(await calls('score')).toEqual([
      { body: { nationalId: 'CM-1', nested: { region: 'CM' } }, authenticated: true, mode: 'score' },
    ]);
    for (const item of cases) {
      const response = await evaluate(item.name);
      expect(response.status()).toBe(item.outcome === 'error' ? 400 : 200);
      const body = await response.json();
      expect(JSON.stringify(body)).not.toContain('feature-test-secret');
      if (item.outcome === 'error') {
        expect(JSON.stringify(body)).toContain('the service answered 503');
      } else {
        expect(body.result.bureau.score).toBe(item.score);
        expect(body.trace.bureau.traceData).toMatchObject({
          mode: 'live',
          outcome: item.outcome,
          ...(item.attempts > 0 ? { attempts: item.attempts } : {}),
          ...(item.error ? { error: { code: item.error } } : {}),
        });
      }
      const received = await calls(item.name);
      expect(received).toHaveLength(item.attempts);
      for (const call of received)
        expect(call).toMatchObject({ authenticated: true, body: { nationalId: 'CM-1', nested: { region: 'CM' } } });
    }
    for (let failure = 0; failure < 5; failure++) {
      const response = await evaluate('breaker');
      await expect(response).toBeOK();
      expect((await response.json()).trace.bureau.traceData).toMatchObject({
        outcome: 'fallback',
        attempts: 1,
        error: { code: 'status' },
      });
    }
    const paused = await evaluate('breaker');
    await expect(paused).toBeOK();
    expect((await paused.json()).trace.bureau.traceData).toMatchObject({
      outcome: 'fallback',
      error: { code: 'circuit_open' },
    });
    expect(await calls('breaker')).toHaveLength(5);
  } finally {
    await expect(await admin.delete(`${path}/environments/staging/tokens/${token.id}`)).toBeOK();
  }
});

test('a connector set up with OAuth2 and a client certificate stores names only and simulates from its mock', async ({
  context,
  admin,
  messages: m,
  appLocale,
}) => {
  await expect(await admin.put('/api/v1/me/tours/editor')).toBeOK();
  const { path, key } = await ruleProject(admin, 'core-banking');
  const added = await admin.post(`${path}/decisions`, {
    data: { key: 'accounts', content: connectorGraph({ preset: 'http' }) },
  });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  const decisionPath = `${path}/decisions/${decision.id}`;
  const page = await openPage(context, `/${appLocale}/projects/decision?p=${key}&d=accounts`);
  await page.getByRole('button', { name: m.connector.settings, exact: true }).click();
  const sheet = page.getByRole('dialog', { name: m.connector.title, exact: true });
  await sheet.getByLabel(m.connector.url, { exact: true }).fill('https://cbs.bank.example/accounts');
  await sheet.getByRole('combobox', { name: m.connector.authType }).click();
  await page.getByRole('option', { name: m.connector.auth.oauth2, exact: true }).click();
  await sheet.getByLabel(m.connector.tokenUrl, { exact: true }).fill('https://login.bank.example/oauth2/token');
  await sheet.getByLabel(m.connector.clientId, { exact: true }).fill('donka-runtime');
  await sheet.getByLabel(m.connector.scopes, { exact: true }).fill('accounts.read');
  await sheet.getByLabel(m.connector.clientSecret, { exact: true }).fill('CBS_CLIENT_SECRET');
  await sheet.getByLabel(m.connector.tls, { exact: true }).check();
  await sheet.getByLabel(m.connector.tlsSecret.certificate, { exact: true }).fill('cbs cert');
  await sheet.getByLabel(m.connector.tlsSecret.key, { exact: true }).fill('CBS_CLIENT_KEY');
  await sheet.getByLabel(m.connector.mock, { exact: true }).fill('{"balance":1200}');
  await sheet.getByRole('button', { name: m.connector.apply, exact: true }).click();
  await expect(sheet.getByText(m.validation.secretName, { exact: true })).toBeVisible();
  await sheet.getByLabel(m.connector.tlsSecret.certificate, { exact: true }).fill('CBS_CLIENT_CERT');
  await sheet.getByRole('button', { name: m.connector.apply, exact: true }).click();
  await expect(sheet).toBeHidden();
  let config: Record<string, unknown> = {};
  await expect
    .poll(async () => {
      const body = await (await admin.get(decisionPath)).json();
      config = body.content.nodes.find((node: { id: string }) => node.id === 'bureau').content.config;
      return config.auth;
    })
    .toEqual({
      type: 'oauth2',
      tokenUrl: 'https://login.bank.example/oauth2/token',
      clientId: 'donka-runtime',
      secret: 'CBS_CLIENT_SECRET',
      scopes: ['accounts.read'],
    });
  expect(config.tls).toEqual({ certificate: 'CBS_CLIENT_CERT', key: 'CBS_CLIENT_KEY' });
  // Simulation answers from the mock: neither address is called (they do not exist).
  const simulated = await admin.post(`${decisionPath}/simulate`, { data: { context: {} } });
  await expect(simulated).toBeOK();
  expect(await simulated.json()).toMatchObject({ result: { response: { balance: 1200 } } });
});
