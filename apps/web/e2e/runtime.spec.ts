import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect } from './fixtures';
import { openPage } from './navigation';
import { expressionGraph, issueToken, ruleProject, snapshot, stage, version } from './release-fixtures';
import { origin, settings } from './settings';

test('Runtime tokens isolate projects and environments, revoke live and have independent rate budgets', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  await expect(await admin.put('/api/v1/me/tours/environments')).toBeOK();
  const projects = [];
  for (const prefix of ['runtime-a', 'runtime-b']) {
    const project = await ruleProject(admin, prefix);
    const added = await admin.post(`${project.path}/decisions`, {
      data: { key: 'calculate', content: expressionGraph(2) },
    });
    await expect(added).toBeOK();
    const decision: ApiSchemas['DecisionResponse'] = await added.json();
    await version(admin, `${project.path}/decisions/${decision.id}`, 'Runtime boundary fixture');
    const release = await snapshot(admin, project.path, 'Runtime boundary fixture');
    projects.push({ ...project, releaseId: release.id });
  }
  const [project, other] = projects;
  if (!project || !other) throw new Error('Both isolated Runtime projects are required');
  const primary = await issueToken(admin, project.path, 'staging', 'Revocable caller');
  const independent = await issueToken(admin, project.path, 'staging', 'Independent caller');
  const production = await issueToken(admin, project.path, 'production', 'Production-only caller');
  const otherToken = await issueToken(admin, other.path, 'staging', 'Other project caller');
  // All tokens are in the first artifact observed by both staging instances.
  for (const item of projects) await stage(admin, item.path, item.releaseId);
  const evaluate = (runtime: string, key: string, token: string) =>
    services.post(`${runtime}/api/projects/${key}/evaluate/calculate`, {
      headers: { 'X-Access-Token': token },
      data: { context: { input: 12 } },
    });
  const runtime = settings().runtime;
  let revoked = false;
  try {
    await expect
      .poll(async () => (await evaluate(runtime, project.key, primary.token)).status(), { timeout: 60_000 })
      .toBe(200);
    await expect
      .poll(async () => (await evaluate(runtime, other.key, otherToken.token)).status(), { timeout: 60_000 })
      .toBe(200);
    for (const [key, token] of [
      [other.key, primary.token],
      [project.key, otherToken.token],
      [project.key, production.token],
      [project.key, 'invalid-runtime-token'],
    ] as const) {
      expect((await evaluate(runtime, key, token)).status()).toBe(401);
    }
    // An invalid token observes loader readiness without consuming a valid token's allowance.
    const limited = origin('DONKA_E2E_LIMITED_RUNTIME_URL');
    await expect
      .poll(async () => (await evaluate(limited, project.key, 'invalid-runtime-token')).status(), { timeout: 60_000 })
      .toBe(401);
    for (let request = 0; request < 3; request++)
      await expect(await evaluate(limited, project.key, primary.token)).toBeOK();
    const throttled = await evaluate(limited, project.key, primary.token);
    expect(throttled.status()).toBe(429);
    expect(Number(throttled.headers()['retry-after'])).toBeGreaterThan(0);
    const rules = await services.get(`${limited}/api/rules/${project.key}`, {
      headers: { 'X-Access-Token': primary.token },
    });
    expect(rules.status()).toBe(429);
    expect(await rules.json()).toMatchObject({ code: 'rateLimit.exceeded', retryAfter: expect.any(Number) });
    expect(Number(rules.headers()['retry-after'])).toBeGreaterThan(0);
    await expect(await evaluate(limited, project.key, independent.token)).toBeOK();
    expect((await evaluate(limited, project.key, 'invalid-runtime-token')).status()).toBe(401);
    await expect(await services.get(`${limited}/api/health`)).toBeOK();
    let page = await openPage(context, `/${appLocale}/projects/environments?p=${project.key}`);
    const staging = () => page.getByRole('region', { name: m.environments.staging, exact: true });
    const tokenRow = () => staging().getByRole('listitem').filter({ hasText: 'Revocable caller' });
    await tokenRow().getByRole('button', { name: m.tokens.revoke, exact: true }).click();
    await page.getByRole('alertdialog').getByRole('button', { name: m.common.cancel, exact: true }).click();
    await expect(await evaluate(runtime, project.key, primary.token)).toBeOK();
    await tokenRow().getByRole('button', { name: m.tokens.revoke, exact: true }).click();
    await page.getByRole('alertdialog').getByRole('button', { name: m.tokens.revoke, exact: true }).click();
    await expect(tokenRow().getByText(m.tokens.revokedBadge, { exact: true })).toBeVisible();
    revoked = true;
    await expect
      .poll(async () => (await evaluate(runtime, project.key, primary.token)).status(), { timeout: 60_000 })
      .toBe(401);
    await expect
      .poll(async () => (await evaluate(limited, project.key, primary.token)).status(), { timeout: 60_000 })
      .toBe(401);
    await expect(await evaluate(runtime, project.key, independent.token)).toBeOK();
    // A fresh page must only show token names/fingerprints, never recover the shown-once value.
    page = await openPage(context, page.url(), page);
    await expect(tokenRow().getByText(m.tokens.revokedBadge, { exact: true })).toBeVisible();
    expect(await page.content()).not.toContain(independent.token);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.evaluate(() => window.scrollTo(0, 0));
    await testInfo.attach('runtime-token-revocation', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
  } finally {
    for (const [path, environment, token] of [
      [project.path, 'staging', primary],
      [project.path, 'staging', independent],
      [project.path, 'production', production],
      [other.path, 'staging', otherToken],
    ] as const) {
      if (token.id === primary.id && revoked) continue;
      await expect(await admin.delete(`${path}/environments/${environment}/tokens/${token.id}`)).toBeOK();
    }
  }
});
