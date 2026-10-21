import { readFile } from 'node:fs/promises';
import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect } from './fixtures';
import { openPage } from './navigation';
import { recordProject, recordViewer } from './record-fixtures';
import { ruleProject } from './release-fixtures';

test('owners filter and export real record audit actions; viewers read records but cannot read audit or change settings', async ({
  context,
  reviewer,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const fixture = await recordProject(admin, services);
  const { key, path } = fixture;
  let signedIn = false;
  try {
    const record = await fixture.emit();
    await fixture.received(record.reference);
    let page = await openPage(context, `/${appLocale}/projects/decision-record?p=${key}&r=${record.id}`);
    await expect(page.getByRole('heading', { name: record.reference, exact: true })).toBeVisible();
    await page.getByRole('button', { name: m.decisionLog.replay, exact: true }).click();
    await expect(page.getByText(m.decisionLog.identicalTitle, { exact: true })).toBeVisible();
    page = await openPage(context, `/${appLocale}/projects/audit?p=${key}`, page);
    const viewed = m.auditSentences.decisionRecordViewed
      .replace('{key}', 'score')
      .replace('{reference}', record.reference);
    const replayed = m.auditSentences.decisionRecordReplayed
      .replace('{key}', 'score')
      .replace('{reference}', record.reference);
    await expect(page.getByText(viewed, { exact: true })).toBeVisible();
    await expect(page.getByText(replayed, { exact: true })).toBeVisible();
    await page.getByRole('combobox', { name: m.audit.action, exact: true }).click();
    await page.getByRole('option', { name: m.auditActions.decision_record.viewed, exact: true }).click();
    const meResponse = await admin.get('/api/v1/auth/me');
    await expect(meResponse).toBeOK();
    const me: ApiSchemas['UserResponse'] = await meResponse.json();
    await page.getByRole('combobox', { name: m.audit.person, exact: true }).click();
    await page.getByPlaceholder(m.audit.searchPerson, { exact: true }).fill(me.email);
    await page.getByRole('option', { name: me.email, exact: true }).click();
    await expect(page.getByRole('table', { name: m.audit.title, exact: true }).getByRole('row')).toHaveCount(2);
    await expect(page.getByText(viewed, { exact: true })).toBeVisible();
    await expect(page.getByText(replayed, { exact: true })).toHaveCount(0);
    const download = page.waitForEvent('download');
    await page.getByRole('link', { name: m.audit.export, exact: true }).click();
    const exported = await download;
    expect(exported.suggestedFilename()).toBe(`audit-${key}.csv`);
    const file = await exported.path();
    if (!file) throw new Error('The audit CSV was not downloaded');
    const csv = await readFile(file, 'utf8');
    expect(csv).toContain('occurred_at,actor,action,target,details');
    expect(csv).toContain('decision_record.viewed');
    expect(csv).toContain(record.reference);
    expect(csv).not.toContain('decision_record.replayed');
    expect(csv.trim().split('\r\n')).toHaveLength(2);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await testInfo.attach('record-audit', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
    const viewer = await recordViewer(admin, services, path, appLocale);
    const foreign = await ruleProject(admin, 'private-records');
    let member = await openPage(reviewer, `/${appLocale}/sign-in`);
    await member.getByLabel(m.signIn.email, { exact: true }).fill(viewer.email);
    await member.getByLabel(m.signIn.password, { exact: true }).fill(viewer.password);
    await member.getByRole('button', { name: m.signIn.submit, exact: true }).click();
    await expect(member).toHaveURL(new RegExp(`/${appLocale}/?$`));
    signedIn = true;
    member = await openPage(reviewer, `/${appLocale}/projects/decision-record?p=${key}&r=${record.id}`, member);
    await expect(member.getByRole('heading', { name: record.reference, exact: true })).toBeVisible();
    await member.getByRole('button', { name: m.decisionLog.replay, exact: true }).click();
    await expect(member.getByText(m.decisionLog.identicalTitle, { exact: true })).toBeVisible();
    member = await openPage(reviewer, `/${appLocale}/projects/audit?p=${key}`, member);
    await expect(member.getByText(m.audit.ownersOnly, { exact: true })).toBeVisible();
    await expect(member.getByRole('link', { name: m.audit.export, exact: true })).toHaveCount(0);
    for (const suffix of ['/audit', '/audit/export']) {
      const response = await reviewer.request.get(`${path}${suffix}`);
      expect(response.status()).toBe(403);
      expect((await response.json()).code).toBe('FORBIDDEN');
    }
    const denied = await reviewer.request.put(`${path}/decision-log/settings`, {
      headers: { 'x-donka-csrf': '1' },
      data: { outcomeField: 'must.not.change', redactedFields: [] },
    });
    expect(denied.status()).toBe(403);
    expect((await denied.json()).code).toBe('FORBIDDEN');
    expect((await reviewer.request.get(`${foreign.path}/decision-log/${record.id}`)).status()).toBe(404);
    member = await openPage(reviewer, `/${appLocale}/projects/settings?p=${key}`, member);
    await expect(member.getByLabel(m.decisionLog.outcomeField, { exact: true })).toBeDisabled();
    await expect(member.getByLabel(m.decisionLog.redactedFields, { exact: true })).toBeDisabled();
    const events = await admin.get(`${path}/audit`, { params: { actor: viewer.userId } });
    await expect(events).toBeOK();
    const actions: ApiSchemas['AuditListResponse'] = await events.json();
    expect(actions.items.map((event) => event.action)).toEqual(
      expect.arrayContaining(['decision_record.viewed', 'decision_record.replayed']),
    );
    expect(await fixture.calls()).toHaveLength(1);
  } finally {
    if (signedIn)
      await expect(await reviewer.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
    await fixture.cleanup();
  }
});
