import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, unique } from './fixtures';
import { openPage } from './navigation';
import { saveVersion } from './decision-actions';

test('saving a table reports a row that is never used, and an owner acknowledges it with a reason', async ({
  context,
  admin,
  messages: m,
  appLocale,
}, testInfo) => {
  await expect(await admin.put('/api/v1/me/tours/editor')).toBeOK();
  const key = unique('table-checks');
  const created = await admin.post('/api/v1/projects', { data: { key, name: 'Threshold rules', description: '' } });
  await expect(created).toBeOK();
  const project: ApiSchemas['ProjectResponse'] = await created.json();
  const path = `/api/v1/projects/${project.id}`;
  // The engine fixture's table, with a row "> 20" that "> 10" above it always answers first.
  const graph = JSON.parse(
    await readFile(resolve(dirname(testInfo.file), '../../../crates/engine/tests/fixtures/table.json'), 'utf8'),
  );
  graph.nodes[1].content.rules.splice(1, 0, { _id: 'over-20', xWauegxfG7: '> 20', qGAHmak0xj: '20' });
  const added = await admin.post(`${path}/decisions`, { data: { key: 'thresholds', content: graph } });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  const decisionPath = `${path}/decisions/${decision.id}`;

  const page = await openPage(context, `/${appLocale}/projects/decision?p=${key}&d=thresholds`);
  // The finding is in the toolbar before anything is saved…
  const checks = page.getByRole('button', { name: new RegExp(`^${m.tableChecks.button}`) });
  await expect(checks).toContainText('1');

  // …and saving a version goes through, then says what the table check found.
  const saved = await saveVersion(page, decisionPath, 'Over 20', {
    save: m.versions.save,
    message: m.versions.message,
  });
  expect(saved.findings.map((finding) => [finding.kind, finding.row])).toEqual([['overlap', 2]]);
  const alert = page.getByRole('status').filter({ hasText: m.tableChecks.review });
  await expect(alert).toBeVisible();
  await alert.getByRole('button', { name: m.tableChecks.review, exact: true }).click();

  const sheet = page.getByRole('dialog', { name: m.tableChecks.title, exact: true });
  const finding = sheet.getByRole('listitem').filter({ hasText: 'Hello' });
  await expect(finding).toHaveCount(1);
  await expect(finding).toContainText('{"input":');
  await finding.getByRole('button', { name: m.tableChecks.acknowledge, exact: true }).click();
  const dialog = page.getByRole('dialog', { name: m.tableChecks.acknowledgeTitle, exact: true });
  await dialog.getByLabel(m.tableChecks.reason).fill('Scores over 20 are capped upstream');
  const acknowledged = page.waitForResponse(
    (reply) => reply.url().endsWith('/acknowledgement') && reply.request().method() === 'PUT',
  );
  await dialog.getByRole('button', { name: m.tableChecks.acknowledge, exact: true }).click();
  expect((await acknowledged).ok()).toBe(true);
  await expect(dialog).toBeHidden();
  await expect(finding).toContainText('Scores over 20 are capped upstream');
  await expect(finding.getByRole('button', { name: m.tableChecks.acknowledge, exact: true })).toHaveCount(0);

  // The draft's findings and the audit log keep the reason.
  const findings = await (await admin.get(`${decisionPath}/findings`)).json();
  expect(findings.items[0].acknowledgement.reason).toBe('Scores over 20 are capped upstream');
  const audit = await admin.get(`${path}/audit?action=decision.finding_acknowledged`);
  await expect(audit).toBeOK();
  expect((await audit.json()).total).toBe(1);
});
