import { z } from 'zod';
import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, newUser, unique } from './fixtures';
import { openPage } from './navigation';
import { importCatalogue, nameProject } from './pack-actions';
import { issueToken, snapshot, stage } from './release-fixtures';
import { settings } from './settings';

const graph = z.object({ nodes: z.array(z.record(z.string(), z.unknown())) }).passthrough();
function renamed(content: unknown, name: string) {
  const changed = graph.parse(structuredClone(content));
  const input = changed.nodes[0];
  if (!input) throw new Error('The imported graph has no input');
  input.name = name;
  return changed;
}

test('catalogue, draft/release copies and exported pack files are independent; invalid files and key conflicts are refused', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const catalogue: ApiSchemas['PackListResponse'] = await (await admin.get('/api/v1/packs')).json();
  expect(catalogue.items.map((pack) => pack.key).sort()).toEqual([
    'kyc-risk-rating',
    'mobile-money-limits',
    'retail-credit',
    'sme-treasury',
  ]);
  const pack = catalogue.items.find((item) => item.key === 'mobile-money-limits');
  if (!pack) throw new Error('The mobile money catalogue pack is missing');
  const key = unique('pack');
  let page = await openPage(context, `/${appLocale}`);
  await page.getByRole('button', { name: m.packs.fromPack, exact: true }).first().click();
  const imported = await importCatalogue(
    page,
    appLocale === 'fr' ? (pack.name.fr ?? pack.name.en) : pack.name.en,
    key,
    'Pack verification',
    m,
  );
  const path = `/api/v1/projects/${imported.project.id}`;
  const list: ApiSchemas['DecisionListResponse'] = await (await admin.get(`${path}/decisions`)).json();
  const decision = list.items[0];
  if (!decision) throw new Error('The imported pack contains no decision');
  const decisionPath = `${path}/decisions/${decision.id}`;
  const original: ApiSchemas['DecisionResponse'] = await (await admin.get(decisionPath)).json();
  const scenarios: ApiSchemas['ScenarioListResponse'] = await (await admin.get(`${path}/test-scenarios`)).json();
  const sample = scenarios.items[0];
  if (!sample) throw new Error('The imported pack contains no scenario');
  expect(scenarios.total).toBe(pack.scenarios);
  await expect(
    await admin.put(`${path}/decision-log/settings`, {
      data: { outcomeField: 'decision', redactedFields: ['wallet.accountId'] },
    }),
  ).toBeOK();
  const member = await newUser(admin, services, appLocale);
  await expect(await admin.post(`${path}/members`, { data: { email: member.email, role: 'viewer' } })).toBeOK();
  const release = await snapshot(admin, path, 'Frozen pack source');
  const token = await issueToken(admin, path, 'staging', 'Pack source caller');
  let pipeline: ApiSchemas['IssuedCiTokenResponse'] | undefined;
  try {
    const issued = await admin.post(`${path}/ci-tokens`, { data: { name: 'Pack source pipeline' } });
    await expect(issued).toBeOK();
    pipeline = await issued.json();
    await stage(admin, path, release.id);
    const reference = unique('pack-source');
    const evaluate = () =>
      services.post(`${settings().runtime}/api/projects/${key}/evaluate/${decision.key}`, {
        headers: { 'X-Access-Token': token.token, 'X-Donka-Reference': reference },
        data: { context: sample.input },
      });
    await expect.poll(async () => (await evaluate()).status(), { timeout: 60_000 }).toBe(200);
    await expect.poll(async () => (await (await admin.get(`${path}/decision-log`)).json()).total).toBeGreaterThan(0);
    await expect(
      await admin.put(decisionPath, {
        data: { revision: original.revision, content: renamed(original.content, 'Edited request') },
      }),
    ).toBeOK();
    const edited: ApiSchemas['DecisionResponse'] = await (await admin.get(decisionPath)).json();

    async function copied(copy: ApiSchemas['ProjectResponse'], content: unknown) {
      const target = `/api/v1/projects/${copy.id}`;
      const decisions: ApiSchemas['DecisionListResponse'] = await (await admin.get(`${target}/decisions`)).json();
      const first = decisions.items[0];
      if (!first) throw new Error('The copied decision is missing');
      const actual: ApiSchemas['DecisionResponse'] = await (await admin.get(`${target}/decisions/${first.id}`)).json();
      expect(actual.content).toEqual(content);
      const results: ApiSchemas['TestResultListResponse'] = await (
        await admin.get(`${target}/decisions/${first.id}/versions/1/test-results`)
      ).json();
      expect(results.items).toHaveLength(pack?.scenarios ?? 0);
      expect(results.items.every((result) => result.status === 'passed')).toBe(true);
      expect(first.id).not.toBe(decision?.id);
      expect((await (await admin.get(`${target}/test-scenarios`)).json()).total).toBe(pack?.scenarios);
      expect(await (await admin.get(`${target}/decision-log/settings`)).json()).toMatchObject({
        outcomeField: 'decision',
        redactedFields: ['wallet.accountId'],
      });
      for (const suffix of ['/releases', '/ci-tokens', '/decision-log'])
        expect((await (await admin.get(`${target}${suffix}`)).json()).items).toEqual([]);
      const environments: ApiSchemas['EnvironmentListResponse'] = await (
        await admin.get(`${target}/environments`)
      ).json();
      for (const environment of environments.items) {
        expect(environment.live ?? null).toBeNull();
        expect(
          (await (await admin.get(`${target}/environments/${environment.environment}/tokens`)).json()).items,
        ).toEqual([]);
      }
      const members: ApiSchemas['MemberListResponse'] = await (await admin.get(`${target}/members`)).json();
      expect(members.items).toHaveLength(1);
      expect(members.items[0]?.role).toBe('owner');
      const audit: ApiSchemas['AuditListResponse'] = await (await admin.get(`${target}/audit`)).json();
      expect(audit.items.find((event) => event.action === 'project.created')?.details).toHaveProperty('from.kind');
      return { path: target, decision: actual };
    }

    page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
    await page.getByRole('button', { name: m.packs.duplicate, exact: true }).click();
    let dialog = page.getByRole('dialog');
    const draftKey = unique('pack-draft');
    await nameProject(dialog, 'Independent draft copy', draftKey, m.projects);
    await testInfo.attach('pack-copy', {
      body: await page.screenshot({ animations: 'disabled' }),
      contentType: 'image/png',
    });
    const duplicated = page.waitForResponse(
      (reply) => reply.url().endsWith(`${path}/duplicate`) && reply.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: m.packs.duplicate, exact: true }).click();
    const draftCopy: ApiSchemas['ImportedResponse'] = await (await duplicated).json();
    await expect(page).toHaveURL(new RegExp(`p=${draftKey}`));
    expect(draftCopy.tests).toMatchObject({ failed: 0, errors: 0, passed: pack.scenarios });
    const draft = await copied(draftCopy.project, edited.content);
    await expect(
      await admin.put(`${draft.path}/decisions/${draft.decision.id}`, {
        data: { revision: draft.decision.revision, content: renamed(draft.decision.content, 'Independent request') },
      }),
    ).toBeOK();
    expect((await (await admin.get(decisionPath)).json()).content).toEqual(edited.content);

    page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
    await page.getByRole('combobox', { name: m.packs.source, exact: true }).click();
    await page
      .getByRole('option', { name: m.packs.release.replace('{version}', release.version), exact: true })
      .click();
    await page.getByRole('button', { name: m.packs.duplicate, exact: true }).click();
    dialog = page.getByRole('dialog');
    const releaseKey = unique('pack-release');
    await nameProject(dialog, 'Independent release copy', releaseKey, m.projects);
    const fromRelease = page.waitForResponse(
      (reply) => reply.url().endsWith(`${path}/duplicate`) && reply.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: m.packs.duplicate, exact: true }).click();
    const releaseCopy: ApiSchemas['ImportedResponse'] = await (await fromRelease).json();
    expect(releaseCopy.tests).toMatchObject({ failed: 0, errors: 0, passed: pack.scenarios });
    await copied(releaseCopy.project, original.content);

    page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
    await page.getByRole('combobox', { name: m.packs.source, exact: true }).click();
    await page
      .getByRole('option', { name: m.packs.release.replace('{version}', release.version), exact: true })
      .click();
    const downloading = page.waitForEvent('download');
    await page.getByRole('button', { name: m.packs.export, exact: true }).click();
    const file = await downloading;
    expect(file.suggestedFilename()).toBe(`${key}.donka-pack.zip`);
    const archive = await file.path();
    if (!archive) throw new Error('The exported pack was not downloaded');
    page = await openPage(context, `/${appLocale}`, page);
    await page.getByRole('button', { name: m.packs.fromPack, exact: true }).first().click();
    dialog = page.getByRole('dialog');
    await dialog.getByRole('tab', { name: m.packs.file, exact: true }).click();
    const upload = dialog.getByLabel(m.packs.fileLabel, { exact: true }).and(dialog.locator('input[type="file"]'));
    await upload.setInputFiles({
      name: 'invalid.zip',
      mimeType: 'application/zip',
      buffer: Buffer.from('not a pack archive'),
    });
    await expect(dialog.getByText(m.errors.INVALID_PACK, { exact: true })).toBeVisible();
    await expect(dialog.getByRole('button', { name: m.packs.import, exact: true })).toBeDisabled();
    await upload.setInputFiles(archive);
    await expect(dialog.getByRole('button', { name: m.packs.import, exact: true })).toBeEnabled();
    await nameProject(dialog, 'Round-trip pack', key, m.projects);
    await dialog.getByRole('button', { name: m.packs.import, exact: true }).click();
    await expect(dialog.getByText(m.errors.PROJECT_KEY_TAKEN, { exact: true })).toBeVisible();
    await expect(dialog.getByLabel(m.projects.name, { exact: true })).toHaveValue('Round-trip pack');
    const roundKey = unique('pack-file');
    await dialog.getByLabel(m.projects.key, { exact: true }).fill(roundKey);
    const fileImported = page.waitForResponse(
      (reply) => new URL(reply.url()).pathname.endsWith('/packs/import') && reply.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: m.packs.import, exact: true }).click();
    expect((await fileImported).ok()).toBe(true);
    await expect(page).toHaveURL(new RegExp(`p=${roundKey}`));
    // File import navigates immediately; independently read persisted content/results through the API.
    await copied(await (await admin.get(`/api/v1/projects/by-key/${roundKey}`)).json(), original.content);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await testInfo.attach('pack-import', {
      body: await page.screenshot({ animations: 'disabled' }),
      contentType: 'image/png',
    });
    expect((await (await admin.get(decisionPath)).json()).content).toEqual(edited.content);
  } finally {
    if (pipeline) await expect(await admin.delete(`${path}/ci-tokens/${pipeline.id}`)).toBeOK();
    await expect(await admin.delete(`${path}/environments/staging/tokens/${token.id}`)).toBeOK();
  }
});
