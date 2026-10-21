import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, unique, passwordLink } from './fixtures';
import { openPage } from './navigation';
import { expressionGraph, issueToken, ruleProject, snapshot, stage, version, waitLive } from './release-fixtures';
import { origin } from './settings';

test('a separate owner rejects and approves production; an owner rolls back only an approved release', async ({
  context,
  reviewer,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  for (const tour of ['releases', 'environments']) await expect(await admin.put(`/api/v1/me/tours/${tour}`)).toBeOK();
  const project = await ruleProject(admin, 'approval');
  const { path, key } = project;
  // Account activation and graph/version creation are explicit API fixtures.
  const email = `${unique('release-owner')}@donka.local`;
  const password = 'disposable release owner passphrase';
  await expect(
    await admin.post('/api/v1/users/invitations', { data: { email, locale: appLocale, isAdmin: false } }),
  ).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', { data: { token: await passwordLink(services, email), password } }),
  ).toBeOK();
  await expect(await admin.post(`${path}/members`, { data: { email, role: 'owner' } })).toBeOK();
  let owner = await openPage(reviewer, `/${appLocale}/sign-in`);
  await owner.getByLabel(m.signIn.email, { exact: true }).fill(email);
  await owner.getByLabel(m.signIn.password, { exact: true }).fill(password);
  await owner.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(owner).toHaveURL(new RegExp(`/${appLocale}/?$`));
  for (const tour of ['releases', 'environments'])
    await expect(await reviewer.request.put(`/api/v1/me/tours/${tour}`, { headers: { 'x-donka-csrf': '1' } })).toBeOK();
  const added = await admin.post(`${path}/decisions`, { data: { key: 'calculate', content: expressionGraph(2) } });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  const decisionPath = `${path}/decisions/${decision.id}`;
  await version(admin, decisionPath, 'Double input');
  const first = await snapshot(admin, path, 'Double input');
  await stage(admin, path, first.id);
  const token = await issueToken(admin, path, 'production', 'Production verification');
  const stagingToken = await issueToken(admin, path, 'staging', 'Staging boundary verification');
  let page = await openPage(context, `/${appLocale}/projects/environments?p=${key}`);
  const production = () => page.getByRole('region', { name: m.environments.production, exact: true });
  async function request(version: string) {
    await production()
      .getByRole('button', { name: m.environments.requestProduction.replace('{version}', version), exact: true })
      .click();
    const dialog = page.getByRole('alertdialog');
    const pending = page.waitForResponse(
      (reply) => reply.url().endsWith(`${path}/approvals`) && reply.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: m.environments.requestAction, exact: true }).click();
    const response = await pending;
    expect(response.ok()).toBe(true);
    const approval: ApiSchemas['ApprovalResponse'] = await response.json();
    await expect(page.getByText(m.approvals.authorCannotDecide, { exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: m.approvals.approve, exact: true })).toHaveCount(0);
    const denied = await admin.post(`${path}/approvals/${approval.id}/approve`);
    expect(denied.status()).toBe(422);
    expect((await denied.json()).code).toBe('SELF_APPROVAL');
    const rejectOwn = await admin.post(`${path}/approvals/${approval.id}/reject`, {
      data: { reason: 'Must refuse self-rejection' },
    });
    expect(rejectOwn.status()).toBe(422);
    expect((await rejectOwn.json()).code).toBe('SELF_APPROVAL');
    owner = await openPage(reviewer, `/${appLocale}/projects/approval?p=${key}&a=${approval.id}`, owner);
    return approval;
  }
  async function approve() {
    await owner.getByRole('button', { name: m.approvals.approve, exact: true }).click();
    await owner.getByRole('alertdialog').getByRole('button', { name: m.approvals.approve, exact: true }).click();
    await expect(owner.getByText(m.approvals.approvedTitle.replace('{email}', email), { exact: true })).toBeVisible();
  }
  const runtime = origin('DONKA_E2E_PRODUCTION_RUNTIME_URL');
  const evaluate = () =>
    services.post(`${runtime}/api/projects/${key}/evaluate/calculate`, {
      headers: { 'X-Access-Token': token.token },
      data: { context: { input: 12 } },
    });
  async function output(releaseId: string, value: number) {
    await expect
      .poll(
        async () => {
          const response = await evaluate();
          if (!response.ok()) return null;
          return (await response.json()).details.releaseId;
        },
        { timeout: 60_000 },
      )
      .toBe(releaseId);
    expect((await (await evaluate()).json()).result).toMatchObject({ output: value });
  }
  try {
    const rejected = await request(first.version);
    await owner.getByRole('button', { name: m.approvals.reject, exact: true }).click();
    let dialog = owner.getByRole('dialog');
    await dialog.getByRole('button', { name: m.approvals.reject, exact: true }).click();
    await expect(dialog.getByText(m.validation.required, { exact: true })).toBeVisible();
    await dialog.getByLabel(m.approvals.reason, { exact: true }).fill('Check the sample boundary before promotion');
    await dialog.getByRole('button', { name: m.approvals.reject, exact: true }).click();
    await expect(owner.getByText('Check the sample boundary before promotion', { exact: true })).toBeVisible();
    const review = await admin.get(`${path}/approvals/${rejected.id}`);
    expect(await review.json()).toMatchObject({ status: 'rejected', decidedBy: { email } });
    expect((await evaluate()).status()).toBe(404);
    const neverApproved = await admin.post(`${path}/rollbacks`, {
      data: { releaseId: first.id, reason: 'Must refuse' },
    });
    expect(neverApproved.status()).toBe(422);
    expect((await neverApproved.json()).code).toBe('NEVER_APPROVED');
    page = await openPage(context, `/${appLocale}/projects/environments?p=${key}`, page);
    await request(first.version);
    await approve();
    await waitLive(admin, path, 'production', first.id);
    await output(first.id, 24);
    const crossed = await services.post(`${runtime}/api/projects/${key}/evaluate/calculate`, {
      headers: { 'X-Access-Token': stagingToken.token },
      data: { context: { input: 12 } },
    });
    expect(crossed.status()).toBe(401);
    await testInfo.attach('production-approval', {
      body: await owner.screenshot({ fullPage: true }),
      contentType: 'image/png',
    });
    // A second version changes observable Runtime behavior; rollback must restore the first.
    const current = await admin.get(decisionPath);
    const saved: ApiSchemas['DecisionResponse'] = await current.json();
    await expect(
      await admin.put(decisionPath, { data: { content: expressionGraph(3), revision: saved.revision } }),
    ).toBeOK();
    await version(admin, decisionPath, 'Triple input');
    const second = await snapshot(admin, path, 'Triple input');
    await stage(admin, path, second.id);
    page = await openPage(context, `/${appLocale}/projects/environments?p=${key}`, page);
    await request(second.version);
    await approve();
    await waitLive(admin, path, 'production', second.id);
    await output(second.id, 36);
    page = await openPage(context, `/${appLocale}/projects/environments?p=${key}`, page);
    await production().getByRole('button', { name: m.rollback.action, exact: true }).click();
    dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('combobox', { name: m.rollback.release, exact: true })).toContainText(first.version);
    await dialog.getByRole('button', { name: m.common.cancel, exact: true }).click();
    await output(second.id, 36);
    await production().getByRole('button', { name: m.rollback.action, exact: true }).click();
    dialog = page.getByRole('dialog');
    await dialog
      .getByRole('button', { name: m.rollback.submit.replace('{version}', first.version), exact: true })
      .click();
    await expect(dialog.getByText(m.validation.required, { exact: true })).toBeVisible();
    await dialog.getByLabel(m.rollback.reason, { exact: true }).fill('Return to the verified double-input policy');
    await dialog
      .getByRole('button', { name: m.rollback.submit.replace('{version}', first.version), exact: true })
      .click();
    await expect(dialog).toBeHidden();
    await waitLive(admin, path, 'production', first.id);
    await output(first.id, 24);
    await expect(production().getByText(m.environments.rolledBack, { exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await testInfo.attach('approved-rollback', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    });
  } finally {
    await expect(await admin.delete(`${path}/environments/production/tokens/${token.id}`)).toBeOK();
    await expect(await admin.delete(`${path}/environments/staging/tokens/${stagingToken.id}`)).toBeOK();
    await expect(await reviewer.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
  }
});
