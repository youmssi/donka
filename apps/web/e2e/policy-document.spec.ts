import { readFile } from 'node:fs/promises';

import type { ApiSchemas } from '../src/components/shared/api';
import { expect, test } from './fixtures';
import { openPage } from './navigation';
import { expressionGraph, ruleProject, snapshot, stage, version } from './release-fixtures';

/** A contract, so the document has fields to list beside the rules. */
const inputSchema = {
  type: 'object',
  properties: {
    input: { type: 'number' },
    region: { type: 'string' },
  },
  required: ['input'],
};

test('a member exports the rules of a release as a document for auditors', async ({
  context,
  admin,
  messages: m,
  appLocale,
}, testInfo) => {
  for (const tour of ['releases', 'environments']) await expect(await admin.put(`/api/v1/me/tours/${tour}`)).toBeOK();
  const project = await ruleProject(admin, 'policy');
  const { path, key } = project;

  const added = await admin.post(`${path}/decisions`, {
    data: { key: 'score', content: expressionGraph(2, inputSchema) },
  });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  await version(admin, `${path}/decisions/${decision.id}`, 'First rules');
  const release = await snapshot(admin, path, 'Ceiling raised for salaried applicants');
  // Live on staging, so the document can say where these rules are in force.
  await stage(admin, path, release.id);

  const page = await openPage(context, `/${appLocale}/projects/releases?p=${key}`);
  await page.getByRole('button', { name: m.releases.actionsFor.replace('{version}', release.version) }).click();
  const downloading = page.waitForEvent('download');
  await page.getByRole('menuitem', { name: m.releases.document, exact: true }).click();
  const downloaded = await downloading;

  // The server names the file after the release it describes.
  expect(downloaded.suggestedFilename()).toBe(`policy-${release.version}.pdf`);
  const file = await downloaded.path();
  if (!file) throw new Error('The policy document was not downloaded');
  const pdf = await readFile(file);
  expect(pdf.subarray(0, 5).toString()).toBe('%PDF-');
  // A document with the rules, the contract, the history and the approvals in it is never a
  // near-empty page; a renderer that lost its content would still start with %PDF-.
  expect(pdf.byteLength).toBeGreaterThan(2_000);

  await expect(
    page.getByText(m.releases.documentReady.replace('{file}', downloaded.suggestedFilename())),
  ).toBeVisible();

  // Reading the policy is recorded, with the release it describes.
  const audited = await admin.get(`${path}/audit`, { params: { action: 'release.document_exported' } });
  await expect(audited).toBeOK();
  const entries = await audited.json();
  expect(entries.total).toBe(1);
  expect(entries.items[0].details.releaseId).toBe(release.id);
  expect(entries.items[0].details.version).toBe(release.version);
  expect(entries.items[0].details.liveIn).toEqual(['staging']);
  expect(entries.items[0].details.language).toBe(appLocale);

  await testInfo.attach('policy-document', {
    body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
    contentType: 'image/png',
  });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
