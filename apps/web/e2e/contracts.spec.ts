import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, unique, passwordLink } from './fixtures';
import { openPage } from './navigation';
import { saveVersion } from './decision-actions';
import { settings } from './settings';

test('input field types, validation and raw schema survive a fresh editor load', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  await expect(await admin.put('/api/v1/me/tours/editor')).toBeOK();
  const key = unique('contracts');
  const created = await admin.post('/api/v1/projects', {
    data: { key, name: 'Contract verification', description: '' },
  });
  await expect(created).toBeOK();
  const project: ApiSchemas['ProjectResponse'] = await created.json();
  const path = `/api/v1/projects/${project.id}`;
  // This journey tests contracts; the graph is an explicitly seeded engine fixture.
  const graph: unknown = JSON.parse(
    await readFile(resolve(dirname(testInfo.file), '../../../crates/engine/tests/fixtures/table.json'), 'utf8'),
  );
  const added = await admin.post(`${path}/decisions`, { data: { key: 'table', content: graph } });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  const decisionPath = `${path}/decisions/${decision.id}`;
  let page = await openPage(context, `/${appLocale}/projects/decision?p=${key}&d=table`);
  await page.getByRole('button', { name: m.inputFields.open }).click();
  const fields = [
    { path: 'input', type: 'number', minimum: '0', maximum: '100' },
    { path: 'age', type: 'integer', minimum: '18', maximum: '99' },
    { path: 'category', type: 'string', values: 'public, private' },
    { path: 'consent', type: 'boolean' },
  ] as const;
  for (const field of fields) {
    await page.getByRole('button', { name: m.inputFields.add, exact: true }).click();
    const dialog = page.getByRole('dialog', { name: m.inputFields.addTitle, exact: true });
    await dialog.getByLabel(m.inputFields.field, { exact: true }).fill(field.path);
    await dialog.getByRole('combobox', { name: m.inputFields.type, exact: true }).click();
    await page.getByRole('option', { name: m.inputFields.types[field.type], exact: true }).click();
    if ('minimum' in field) {
      await dialog.getByLabel(m.inputFields.minimum, { exact: true }).fill(field.minimum);
      await dialog.getByLabel(m.inputFields.maximum, { exact: true }).fill('-1');
      await dialog.getByRole('button', { name: m.inputFields.saveField, exact: true }).click();
      await expect(dialog.getByText(m.validation.minAboveMax, { exact: true })).toBeVisible();
      await dialog.getByLabel(m.inputFields.maximum, { exact: true }).fill(field.maximum);
    }
    if ('values' in field) {
      await dialog.getByLabel(m.inputFields.values, { exact: true }).fill(field.values);
      await dialog.getByLabel(m.inputFields.minLength, { exact: true }).fill('3');
      await dialog.getByLabel(m.inputFields.maxLength, { exact: true }).fill('20');
      await dialog.getByLabel(m.inputFields.labelEn, { exact: true }).fill('Category');
      await dialog.getByLabel(m.inputFields.labelFr, { exact: true }).fill('Catégorie');
      await dialog.getByLabel(m.inputFields.piiLong, { exact: true }).check();
      await dialog.getByLabel(m.inputFields.required, { exact: true }).uncheck();
    }
    await dialog.getByRole('button', { name: m.inputFields.saveField, exact: true }).click();
    await expect(dialog).toBeHidden();
  }
  const sheet = page.getByRole('dialog', { name: m.inputFields.title, exact: true });
  await sheet.getByRole('tab', { name: m.inputFields.jsonTab, exact: true }).click();
  const raw = sheet.getByLabel(m.inputFields.jsonLabel, { exact: true });
  const schema = JSON.parse(await raw.inputValue());
  expect(schema).toMatchObject({
    required: ['input', 'age', 'consent'],
    properties: {
      input: { type: 'number', minimum: 0, maximum: 100 },
      age: { type: 'integer', minimum: 18, maximum: 99 },
      category: {
        type: 'string',
        enum: ['public', 'private'],
        minLength: 3,
        maxLength: 20,
        'x-donka': { label: { en: 'Category', fr: 'Catégorie' }, pii: true },
      },
      consent: { type: 'boolean' },
    },
  });
  await raw.fill('{ broken');
  await sheet.getByRole('button', { name: m.inputFields.applyJson, exact: true }).click();
  await expect(raw).toHaveAttribute('aria-invalid', 'true');
  schema.properties.tags = { type: 'array', items: { type: 'string', pattern: '^[A-Z]+$' } };
  await raw.fill(JSON.stringify(schema, null, 2));
  await sheet.getByRole('button', { name: m.inputFields.applyJson, exact: true }).click();
  await expect
    .poll(async () => {
      const saved = await admin.get(decisionPath);
      await expect(saved).toBeOK();
      const body: ApiSchemas['DecisionResponse'] = await saved.json();
      return JSON.stringify(body.content);
    })
    .toContain('^[A-Z]+$');
  page = await openPage(context, page.url(), page);
  await page.getByRole('button', { name: m.inputFields.open }).click();
  await expect(page.getByText(m.inputFields.jsonOnly, { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: m.inputFields.jsonTab, exact: true }).click();
  expect(JSON.parse(await page.getByLabel(m.inputFields.jsonLabel, { exact: true }).inputValue())).toEqual(schema);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await testInfo.attach('input-contract', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
  await page.keyboard.press('Escape');
  await saveVersion(page, decisionPath, 'Typed input contract', m.versions);
  await expect(page.getByText(m.inputFields.unread.replace('{path}', 'age'), { exact: true })).toBeVisible();
  // Publication and the approval request are fixtures for the real contract comparison screen.
  // Approval decisions and second-person authorization have their own feature journey.
  const released = await admin.post(`${path}/releases`, { data: { bump: 'minor', notes: 'Contract review fixture' } });
  await expect(released).toBeOK();
  const release: ApiSchemas['ReleaseResponse'] = await released.json();
  await expect(
    await admin.post(`${path}/environments/staging/deployments`, { data: { releaseId: release.id } }),
  ).toBeOK();
  await expect
    .poll(
      async () => {
        const response = await admin.get(`${path}/environments`);
        await expect(response).toBeOK();
        const body: ApiSchemas['EnvironmentListResponse'] = await response.json();
        return body.items.find((item) => item.environment === 'staging')?.live?.releaseId;
      },
      { timeout: 60_000 },
    )
    .toBe(release.id);
  await expect(await services.get(`${settings().runtime}/api/health`)).toBeOK();
  const issued = await admin.post(`${path}/environments/staging/tokens`, { data: { name: 'Contract verification' } });
  await expect(issued).toBeOK();
  const token: ApiSchemas['IssuedTokenResponse'] = await issued.json();
  try {
    const valid = { input: 12, age: 30, category: 'public', consent: true, tags: ['OK'] };
    const evaluate = (input: unknown) =>
      services.post(`${settings().runtime}/api/projects/${key}/evaluate/table`, {
        headers: { 'X-Access-Token': token.token },
        data: { context: input },
      });
    await expect.poll(async () => (await evaluate(valid)).status(), { timeout: 60_000 }).toBe(200);
    expect((await (await evaluate(valid)).json()).result).toEqual({ output: 10 });
    for (const [input, field] of [
      [{ ...valid, input: -1 }, 'input'],
      [{ ...valid, input: 101 }, 'input'],
      [{ ...valid, age: 30.5 }, 'age'],
      [{ ...valid, age: 17 }, 'age'],
      [{ ...valid, category: 'other' }, 'category'],
      [{ ...valid, consent: 'yes' }, 'consent'],
      [{ ...valid, tags: ['lowercase'] }, 'tags.0'],
    ] as const) {
      const rejected = await evaluate(input);
      expect(rejected.status()).toBe(400);
      expect(await rejected.json()).toMatchObject({ contract: { field, message: expect.any(String) } });
    }
  } finally {
    await expect(await admin.delete(`${path}/environments/staging/tokens/${token.id}`)).toBeOK();
  }
  const email = `${unique('contract-reviewer')}@donka.local`;
  await expect(
    await admin.post('/api/v1/users/invitations', { data: { email, locale: appLocale, isAdmin: false } }),
  ).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', {
      data: {
        token: await passwordLink(services, email),
        password: 'disposable contract reviewer passphrase',
      },
    }),
  ).toBeOK();
  await expect(await admin.post(`${path}/members`, { data: { email, role: 'owner' } })).toBeOK();
  const requested = await admin.post(`${path}/approvals`, { data: { releaseId: release.id } });
  await expect(requested).toBeOK();
  const approval: ApiSchemas['ApprovalResponse'] = await requested.json();
  const review = await admin.get(`${path}/approvals/${approval.id}`);
  await expect(review).toBeOK();
  const body: ApiSchemas['ApprovalReviewResponse'] = await review.json();
  expect(body.changes[0]?.contract).toEqual(
    expect.arrayContaining([
      { path: 'input', kind: 'added', breaking: true },
      { path: 'category', kind: 'added', breaking: false },
    ]),
  );
  page = await openPage(context, `/${appLocale}/projects/approval?p=${key}&a=${approval.id}`, page);
  const change = page.getByRole('listitem').filter({ has: page.getByRole('link', { name: 'table', exact: true }) });
  await expect(change.getByText(m.approvals.breaking, { exact: true }).first()).toBeVisible();
  await expect(change.getByText(m.approvals.compatible, { exact: true }).first()).toBeVisible();
  await expect(change.getByText('category', { exact: true })).toBeVisible();
  await expect(change.getByText('input', { exact: true })).toBeVisible();
});
