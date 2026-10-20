import type { ApiSchemas } from '../src/components/shared/api';
import type { Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { test, expect, unique } from './fixtures';
import { settings } from './settings';

test('project, field persistence, version and staging release reach the real Runtime', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
  theme,
}, testInfo) => {
  await expect(await services.get(`${settings().runtime}/api/health`)).toBeOK();
  for (const tour of ['editor', 'releases', 'environments']) {
    await expect(await admin.put(`/api/v1/me/tours/${tour}`)).toBeOK();
  }
  // Cold loads also prove deep links and persistence without replacing a departing document.
  // WebKit misclassifies cancelled prefetches during replacement: playwright#43120.
  async function open(url: string, previous?: Page) {
    const opened = await context.newPage();
    await opened.bringToFront();
    await previous?.close();
    await opened.goto(url);
    await expect.poll(() => opened.evaluate(() => document.visibilityState)).toBe('visible');
    return opened;
  }
  const key = unique('browser-rules');
  let page = await open(`/${appLocale}/`);
  await page.getByRole('button', { name: m.projects.create, exact: true }).first().click();
  let dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.projects.name, { exact: true }).fill('Browser rules');
  await dialog.getByLabel(m.projects.key, { exact: true }).fill(key);
  const created = page.waitForResponse(
    (response) => response.url().endsWith('/api/v1/projects') && response.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: m.projects.create, exact: true }).click();
  const projectResponse = await created;
  expect(projectResponse.ok()).toBe(true);
  const project: ApiSchemas['ProjectResponse'] = await projectResponse.json();
  const path = `/api/v1/projects/${project.id}`;
  // Finish the create dialog's redirect before navigating away with the graph fixture.
  await expect(page.getByText(m.decisions.emptyTitle, { exact: true })).toBeVisible();
  // A known valid graph is test data. Authoring graph nodes is a separate coverage story.
  const graph: unknown = JSON.parse(
    await readFile(resolve(dirname(testInfo.file), '../../../crates/engine/tests/fixtures/table.json'), 'utf8'),
  );
  const response = await admin.post(`${path}/decisions`, { data: { key: 'table', content: graph } });
  await expect(response).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await response.json();
  page = await open(`/${appLocale}/projects/decision?p=${key}&d=table`, page);
  await page.getByRole('button', { name: m.inputFields.open }).click();
  await page.getByRole('button', { name: m.inputFields.add, exact: true }).click();
  await page.getByPlaceholder('customer.age', { exact: true }).fill('customer.id');
  await page.getByRole('button', { name: m.inputFields.saveField, exact: true }).click();
  await expect
    .poll(async () => {
      const saved = await admin.get(`${path}/decisions/${decision.id}`);
      await expect(saved).toBeOK();
      return JSON.stringify((await saved.json()).content);
    })
    .toContain('customer');
  await page.keyboard.press('Escape');
  page = await open(page.url(), page);
  // The button's accessible name also includes the saved-field count badge.
  await page.getByRole('button', { name: m.inputFields.open }).click();
  await expect(page.getByRole('cell', { name: /^customer\.id\b/ })).toBeVisible();
  await page.keyboard.press('Escape');

  await page.getByRole('button', { name: m.versions.save, exact: true }).click();
  dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.versions.message, { exact: true }).fill('Browser-verified version');
  const versionSaved = page.waitForResponse(
    (reply) => reply.url().endsWith(`/decisions/${decision.id}/versions`) && reply.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: m.versions.save, exact: true }).click();
  expect((await versionSaved).ok()).toBe(true);
  const previewResponse = await admin.get(`${path}/releases/preview`);
  await expect(previewResponse).toBeOK();
  const preview: ApiSchemas['ReleasePreviewResponse'] = await previewResponse.json();
  page = await open(`/${appLocale}/projects/releases?p=${key}`, page);
  await page.getByRole('button', { name: m.releases.create, exact: true }).first().click();
  dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.releases.notes, { exact: true }).fill('Live browser release');
  const releaseCreated = page.waitForResponse(
    (reply) => reply.url().endsWith(`${path}/releases`) && reply.request().method() === 'POST',
  );
  await dialog
    .getByRole('button', { name: m.releases.createVersion.replace('{version}', preview.next.minor), exact: true })
    .click();
  const releaseResponse = await releaseCreated;
  expect(releaseResponse.ok()).toBe(true);
  const release: ApiSchemas['ReleaseResponse'] = await releaseResponse.json();
  page = await open(`/${appLocale}/projects/environments?p=${key}`, page);
  const staging = page.getByRole('region', { name: m.environments.staging, exact: true });
  const tokenName = 'Browser runtime';
  await staging.getByRole('button', { name: m.tokens.create, exact: true }).click();
  dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.tokens.name, { exact: true }).fill(tokenName);
  const issued = page.waitForResponse(
    (reply) => reply.url().endsWith(`${path}/environments/staging/tokens`) && reply.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: m.tokens.create, exact: true }).click();
  const issuedResponse = await issued;
  expect(issuedResponse.ok()).toBe(true);
  const token: ApiSchemas['IssuedTokenResponse'] = await issuedResponse.json();
  await expect(dialog.getByLabel(tokenName, { exact: true })).toHaveValue(token.token);
  await dialog.getByRole('button', { name: m.tokens.done, exact: true }).click();
  try {
    await staging.getByRole('button', { name: m.environments.deploy, exact: true }).click();
    await expect
      .poll(
        async () => {
          const environments = await admin.get(`${path}/environments`);
          await expect(environments).toBeOK();
          const body: ApiSchemas['EnvironmentListResponse'] = await environments.json();
          return body.items.find((environment) => environment.environment === 'staging')?.live?.releaseId;
        },
        { timeout: 60_000 },
      )
      .toBe(release.id);
    const evaluate = () =>
      services.post(`${settings().runtime}/api/projects/${key}/evaluate/table`, {
        headers: { 'X-Access-Token': token.token },
        data: { context: { input: 12, customer: { id: 'C-1' } } },
      });
    await expect.poll(async () => (await evaluate()).status(), { timeout: 60_000 }).toBe(200);
    expect((await (await evaluate()).json()).result).toEqual({ output: 10 });
    for (const customer of [{}, { id: 42 }]) {
      const invalid = await services.post(`${settings().runtime}/api/projects/${key}/evaluate/table`, {
        headers: { 'X-Access-Token': token.token },
        data: { context: { input: 12, customer } },
      });
      expect(invalid.status()).toBe(400);
      expect(await invalid.json()).toMatchObject({
        contract: { field: 'customer.id', message: expect.any(String) },
      });
    }
    const denied = await services.post(`${settings().runtime}/api/projects/${key}/evaluate/table`, {
      headers: { 'X-Access-Token': 'not-an-issued-token' },
      data: { context: { input: 12 } },
    });
    expect(denied.status()).toBe(401);
    // The UI polls publication independently of the API/Runtime checks above.
    await expect(staging.getByRole('button', { name: m.environments.deploy, exact: true })).toBeDisabled();
    await expect(page.locator('html')).toHaveAttribute('lang', appLocale);
    await expect
      .poll(() => page.locator('html').evaluate((html) => html.classList.contains('dark')))
      .toBe(theme === 'dark');
    await page.evaluate(() => document.fonts.ready);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await testInfo.attach('staging', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
  } finally {
    await expect(await admin.delete(`${path}/environments/staging/tokens/${token.id}`)).toBeOK();
  }
});
