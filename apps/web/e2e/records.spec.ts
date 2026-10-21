import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, unique } from './fixtures';
import { openPage } from './navigation';
import { recordProject } from './record-fixtures';
import { expressionGraph } from './release-fixtures';

test('real Runtime records are paginated and searchable; replay uses recorded connector answers', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const fixture = await recordProject(admin, services);
  const { key, path } = fixture;
  try {
    const answered = await fixture.emit();
    const failed = await fixture.emit('failed');
    await fixture.received(answered.reference);
    await fixture.received(failed.reference);
    expect(answered.body.result.bureau).toMatchObject({ score: 712, available: true });
    expect(await fixture.calls()).toHaveLength(1);
    expect(await fixture.calls('failed')).toHaveLength(1);
    // Deliberately change the editable draft: replay must still load the record's frozen release.
    const current = await admin.get(`${path}/decisions/${fixture.decision.id}`);
    await expect(current).toBeOK();
    await expect(
      await admin.put(`${path}/decisions/${fixture.decision.id}`, {
        data: { revision: (await current.json()).revision, content: expressionGraph(99) },
      }),
    ).toBeOK();
    // Extra records exercise actual UI pagination; none are injected into Studio's feed.
    await Promise.all(Array.from({ length: 51 }, (_, i) => fixture.emit('plain', unique('page'), { input: i })));
    await expect.poll(async () => (await (await admin.get(`${path}/decision-log`)).json()).total).toBe(53);
    let page = await openPage(context, `/${appLocale}/projects/decision-log?p=${key}`);
    const table = () => page.getByRole('table', { name: m.decisionLog.title, exact: true });
    await expect(table().getByRole('row')).toHaveCount(51);
    await page.getByRole('link', { name: m.common.next, exact: true }).click();
    await expect(page).toHaveURL(/offset=50/);
    await expect(table().getByRole('row')).toHaveCount(4);
    await expect(table().getByRole('link').filter({ hasText: answered.reference })).toBeVisible();
    await page.getByRole('link', { name: m.common.previous, exact: true }).click();
    await expect(table().getByRole('row')).toHaveCount(51);
    await page.getByRole('search').getByLabel(m.decisionLog.reference, { exact: true }).fill(answered.reference);
    await page.getByRole('search').getByLabel(m.decisionLog.decision, { exact: true }).fill('score');
    await page.getByRole('search').getByLabel(m.decisionLog.outcome, { exact: true }).fill('712');
    await page.getByRole('button', { name: m.decisionLog.search, exact: true }).click();
    await expect(table().getByRole('row')).toHaveCount(2);
    await page.getByRole('combobox', { name: m.decisionLog.environment, exact: true }).click();
    await page.getByRole('option', { name: m.environments.production, exact: true }).click();
    await expect(page.getByText(m.decisionLog.emptyFilteredTitle, { exact: true })).toBeVisible();
    await page.getByRole('combobox', { name: m.decisionLog.environment, exact: true }).click();
    await page.getByRole('option', { name: m.environments.staging, exact: true }).click();
    await expect(table().getByRole('row')).toHaveCount(2);
    await table().getByRole('link').filter({ hasText: answered.reference }).click();
    await expect(page.getByRole('heading', { name: answered.reference, exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: m.decisionLog.explain, exact: true })).toHaveCount(0);
    expect(
      (await admin.post(`${path}/decision-log/${answered.id}/explain`, { data: { language: appLocale } })).status(),
    ).toBe(404);
    const stored: ApiSchemas['RecordResponse'] = await (await admin.get(`${path}/decision-log/${answered.id}`)).json();
    expect(stored).toMatchObject({
      id: answered.id,
      releaseId: fixture.release.id,
      outcome: '712',
      input: fixture.input,
    });
    expect(stored.trace).toBeTruthy();
    await page.getByText(m.decisionLog.trace, { exact: true }).click();
    await expect(page.locator('details pre')).toContainText('live');
    await page.getByRole('button', { name: m.decisionLog.replay, exact: true }).click();
    await expect(page.getByText(m.decisionLog.identicalTitle, { exact: true })).toBeVisible();
    expect(await fixture.calls()).toHaveLength(1);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const payload = page.locator('pre').filter({ hasText: fixture.input.note }).first();
    await expect(payload).toContainText(fixture.input.note);
    expect(
      await payload.evaluate(
        (element) => element.scrollWidth > element.clientWidth && getComputedStyle(element).overflowX === 'auto',
      ),
    ).toBe(true);
    await page.evaluate(() => window.scrollTo(0, 0));
    await testInfo.attach('record-replay', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
    page = await openPage(context, `/${appLocale}/projects/decision-log?p=${key}`, page);
    await page.getByRole('combobox', { name: m.decisionLog.status, exact: true }).click();
    await page.getByRole('option', { name: m.decisionLog.failed, exact: true }).click();
    await expect(table().getByRole('row')).toHaveCount(2);
    await table().getByRole('link').filter({ hasText: failed.reference }).click();
    await expect(page.getByRole('heading', { name: failed.reference, exact: true })).toBeVisible();
    await expect(page.getByText(m.decisionLog.error, { exact: true })).toBeVisible();
    await page.getByRole('button', { name: m.decisionLog.replay, exact: true }).click();
    await expect(page.getByText(m.decisionLog.identicalTitle, { exact: true })).toBeVisible();
    expect(await fixture.calls('failed')).toHaveLength(1);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.evaluate(() => window.scrollTo(0, 0));
    await testInfo.attach('record-failed-replay', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
    page = await openPage(
      context,
      `/${appLocale}/projects/decision-record?p=${key}&r=00000000-0000-0000-0000-000000000001`,
      page,
    );
    await expect(page.getByText(m.decisionLog.notFoundTitle, { exact: true })).toBeVisible();
  } finally {
    await fixture.cleanup();
  }
});
