import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { z } from 'zod';
import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, newUser, unique } from './fixtures';
import { runCli } from './cli-run';
import { openPage } from './navigation';
import { expressionGraph, issueToken, ruleProject, snapshot, stage, version } from './release-fixtures';
import { settings } from './settings';

const contract = {
  type: 'object',
  required: ['input', 'customer'],
  properties: {
    input: {
      type: 'integer',
      minimum: 0,
      'x-donka': { label: { en: 'Input value', fr: 'Valeur d’entrée' }, order: 1 },
    },
    customer: {
      type: 'object',
      required: ['code'],
      properties: { code: { type: 'string', 'x-donka': { label: { en: 'Customer code', fr: 'Code client' } } } },
    },
  },
};
function content(factor: number) {
  const graph = expressionGraph(factor);
  return {
    ...graph,
    nodes: graph.nodes.map((node) =>
      node.type === 'inputNode' ? { ...node, content: { schema: JSON.stringify(contract) } } : node,
    ),
  };
}
const summary = z.object({
  action: z.literal('load'),
  project: z.string(),
  release: z.string(),
  version: z.string(),
  commit: z.string(),
  sha256: z.string(),
  verified: z.boolean(),
  files: z.array(z.string()),
});

test('owner-issued CI tokens drive real CLI targets, verified artifacts and nested forms; viewers cannot manage them and revocation is immediate', async ({
  context,
  reviewer,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const project = await ruleProject(admin, 'cli');
  const { key, path } = project;
  const added = await admin.post(`${path}/decisions`, { data: { key: 'pricing/score', content: content(2) } });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  const decisionPath = `${path}/decisions/${decision.id}`;
  await version(admin, decisionPath, 'Contract for the CLI');
  const first = await snapshot(admin, path, 'First CLI release');
  const caller = await issueToken(admin, path, 'staging', 'CLI artifact caller');
  const directory = await mkdtemp(join(tmpdir(), 'donka-cli-e2e-'));
  let credential: ApiSchemas['IssuedCiTokenResponse'] | undefined;
  let extra: ApiSchemas['IssuedTokenResponse'] | undefined;
  let signedIn = false;
  let revoked = false;
  try {
    await stage(admin, path, first.id);
    const original: ApiSchemas['DecisionResponse'] = await (await admin.get(decisionPath)).json();
    await expect(
      await admin.put(decisionPath, { data: { revision: original.revision, content: content(3) } }),
    ).toBeOK();
    await version(admin, decisionPath, 'Second CLI release');
    const latest = await snapshot(admin, path, 'Latest CLI release');
    let page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`);
    await page.getByRole('button', { name: m.ciTokens.create, exact: true }).click();
    const dialog = page.getByRole('dialog');
    const tokenName = unique('Pipeline');
    await dialog.getByLabel(m.ciTokens.name, { exact: true }).fill(tokenName);
    const issuing = page.waitForResponse(
      (response) => response.url().endsWith(`${path}/ci-tokens`) && response.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: m.ciTokens.create, exact: true }).click();
    const issued = await issuing;
    expect(issued.ok()).toBe(true);
    const token: ApiSchemas['IssuedCiTokenResponse'] = await issued.json();
    credential = token;
    expect(
      (await dialog.getByLabel(tokenName, { exact: true }).inputValue()) === token.token,
      'Shown-once token matches its issued value',
    ).toBe(true);
    await dialog.getByRole('button', { name: m.ciTokens.done, exact: true }).click();
    async function cli(args: string[], target = 'main', code = 0) {
      const result = await runCli(directory, key, token.token, args, target);
      expect(result.code, `CLI ${args.slice(0, 2).join(' ')} exit code`).toBe(code);
      return result;
    }
    let liveCommit = '';
    for (const [index, [target, release]] of (
      [
        ['main', latest],
        [`commit:${first.id}`, first],
        [`release:${first.version}`, first],
        ['env:staging', first],
      ] as const
    ).entries()) {
      const result = summary.parse(
        JSON.parse((await cli(['pull', '--out', `archive-${index}`, '--json'], target)).stdout),
      );
      expect(result).toMatchObject({ project: key, release: release.id, version: release.version, verified: true });
      const file = result.files[0];
      if (!file) throw new Error('The CLI did not write an artifact');
      expect(basename(file)).toBe(key);
      const checksum = createHash('sha256')
        .update(await readFile(file))
        .digest('hex');
      expect(result.sha256).toBe(checksum);
      const synced = await services.post(`${settings().studio}/api/v1/rules-sync`, {
        headers: { Authorization: `Bearer ${token.token}` },
        data: { deployments: [{ project: key, target }] },
      });
      await expect(synced).toBeOK();
      expect((await synced.json()).deployments[0].artifact.sha256).toBe(checksum);
      const unchanged = await cli(['pull', '--current', result.commit, '--json'], target, 3);
      expect(JSON.parse(unchanged.stdout)).toMatchObject({ action: 'no_change' });
      await cli(['pull', '--unpack', '--out', `unpacked-${index}`, '--json'], target);
      const config = JSON.parse(
        await readFile(join(directory, `unpacked-${index}`, key, '.config/project.json'), 'utf8'),
      );
      expect(config.release.id).toBe(release.id);
      if (target === 'env:staging') {
        liveCommit = result.commit;
        expect(config.environment.key).toBe('staging');
        expect(config.accessTokenHashes).toEqual([
          expect.objectContaining({
            algorithm: 'sha256',
            hash: createHash('sha256').update(caller.token).digest('hex'),
          }),
        ]);
      } else {
        expect(config.environment).toBeUndefined();
        expect(config.accessTokenHashes).toEqual([]);
      }
      const forms = await cli(
        ['form', 'pull', '--decision', decision.key, '--out', `contracts-${index}`, '--json'],
        target,
      );
      expect(JSON.parse(forms.stdout)).toMatchObject({
        release: release.id,
        verified: true,
        decisions: ['pricing/score'],
      });
      expect(
        JSON.parse(await readFile(join(directory, `contracts-${index}/pricing/score/input.schema.json`), 'utf8')),
      ).toEqual(contract);
    }
    const contractFile = join(directory, 'contracts-0/pricing/score/input.schema.json');
    await cli(['form', 'init', '--contract', contractFile, '--lang', appLocale, '--out', 'form.json']);
    const generated = JSON.parse(await readFile(join(directory, 'form.json'), 'utf8'));
    expect(generated).toMatchObject({
      properties: { input: { title: appLocale === 'fr' ? 'Valeur d’entrée' : 'Input value' } },
    });
    expect(
      JSON.parse((await cli(['form', 'check', '--contract', contractFile, '--form', 'form.json', '--json'])).stdout),
    ).toEqual({ ok: true, differences: [] });
    await writeFile(
      join(directory, 'drift.json'),
      JSON.stringify({ ...contract, properties: { ...contract.properties, input: { type: 'string' } } }),
    );
    const drift = await cli(['form', 'check', '--contract', contractFile, '--form', 'drift.json', '--json'], 'main', 1);
    expect(JSON.parse(drift.stdout)).toMatchObject({
      ok: false,
      differences: expect.arrayContaining([expect.objectContaining({ field: 'input', kind: 'type' })]),
    });
    await cli(['pull', '--json'], 'env:production', 4);
    await cli(['pull', '--json'], 'branch:unsupported', 1);
    const foreign = await ruleProject(admin, 'cli-private');
    await cli(['pull', '--project', foreign.key, '--json'], 'main', 1);
    expect((await runCli(directory, key, '', ['pull', '--json'])).code).toBe(2);
    extra = await issueToken(admin, path, 'staging', 'Another CLI artifact caller');
    await expect
      .poll(
        async () => {
          const environments: ApiSchemas['EnvironmentListResponse'] = await (
            await admin.get(`${path}/environments`)
          ).json();
          const staging = environments.items.find((environment) => environment.environment === 'staging');
          return Boolean(staging?.live && staging.live.id !== liveCommit && staging.live.id === staging.latest?.id);
        },
        { timeout: 60_000 },
      )
      .toBe(true);
    const refreshed = summary.parse(
      JSON.parse((await cli(['pull', '--current', liveCommit, '--out', 'refreshed', '--json'], 'env:staging')).stdout),
    );
    expect(refreshed.release).toBe(first.id);
    expect(refreshed.commit).not.toBe(liveCommit);

    const viewer = await newUser(admin, services, appLocale);
    await expect(await admin.post(`${path}/members`, { data: { email: viewer.email, role: 'viewer' } })).toBeOK();
    let member = await openPage(reviewer, `/${appLocale}/sign-in`);
    await member.getByLabel(m.signIn.email, { exact: true }).fill(viewer.email);
    await member.getByLabel(m.signIn.password, { exact: true }).fill(viewer.password);
    await member.getByRole('button', { name: m.signIn.submit, exact: true }).click();
    await expect(member).toHaveURL(new RegExp(`/${appLocale}/?$`));
    signedIn = true;
    member = await openPage(reviewer, `/${appLocale}/projects/settings?p=${key}`, member);
    await expect(member.getByRole('button', { name: m.ciTokens.create, exact: true })).toHaveCount(0);
    expect(
      (
        await reviewer.request.post(`${path}/ci-tokens`, { headers: { 'x-donka-csrf': '1' }, data: { name: 'Denied' } })
      ).status(),
    ).toBe(403);
    expect(
      (await reviewer.request.delete(`${path}/ci-tokens/${token.id}`, { headers: { 'x-donka-csrf': '1' } })).status(),
    ).toBe(403);
    page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
    const row = page.getByRole('listitem').filter({ hasText: tokenName });
    await expect(row.getByText(m.ciTokens.lastUsed, { exact: false })).toBeVisible();
    await row.getByRole('button', { name: m.ciTokens.revoke, exact: true }).click();
    await page.getByRole('alertdialog').getByRole('button', { name: m.ciTokens.revoke, exact: true }).click();
    await expect(row.getByText(m.ciTokens.revokedBadge, { exact: true })).toBeVisible();
    revoked = true;
    await cli(['pull', '--json'], 'main', 1);
    await expect(page.getByRole('dialog')).toHaveCount(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await testInfo.attach('cli-token-revocation', {
      body: await page.screenshot({ animations: 'disabled' }),
      contentType: 'image/png',
    });
  } finally {
    if (signedIn)
      await expect(await reviewer.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
    if (credential && !revoked) await expect(await admin.delete(`${path}/ci-tokens/${credential.id}`)).toBeOK();
    for (const token of [caller, extra])
      if (token) await expect(await admin.delete(`${path}/environments/staging/tokens/${token.id}`)).toBeOK();
    const target = resolve(directory);
    if (dirname(target) !== resolve(tmpdir()) || !basename(target).startsWith('donka-cli-e2e-'))
      throw new Error('Refusing to remove an unexpected test path');
    await rm(target, { recursive: true, force: true });
  }
});
