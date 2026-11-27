import type { ApiSchemas } from '../src/components/shared/api';
import { expect, passwordLink, test, unique } from './fixtures';
import { openPage } from './navigation';
import { expressionGraph, issueToken, ruleProject, snapshot, stage, version, waitLive } from './release-fixtures';
import { origin } from './settings';

/** Both fields the journey measures: a number to bin, and a value to bin by itself. */
const inputSchema = {
  type: 'object',
  properties: {
    input: { type: 'number' },
    region: { type: 'string' },
  },
};

test('an owner measures the decision mix and the population shift of real production decisions', async ({
  context,
  reviewer,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  for (const tour of ['releases', 'environments']) await expect(await admin.put(`/api/v1/me/tours/${tour}`)).toBeOK();
  const project = await ruleProject(admin, 'stability');
  const { path, key } = project;

  // A second owner, because the author of a release cannot approve it.
  const email = `${unique('stability-owner')}@donka.local`;
  const password = 'disposable stability owner passphrase';
  await expect(
    await admin.post('/api/v1/users/invitations', { data: { email, locale: appLocale, isAdmin: false } }),
  ).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', { data: { token: await passwordLink(services, email), password } }),
  ).toBeOK();
  await expect(await admin.post(`${path}/members`, { data: { email, role: 'owner' } })).toBeOK();

  const added = await admin.post(`${path}/decisions`, {
    data: { key: 'score', content: expressionGraph(2, inputSchema) },
  });
  await expect(added).toBeOK();
  const decision: ApiSchemas['DecisionResponse'] = await added.json();
  // The outcome is the value the mix divides by, so the log must know which field it is.
  await expect(
    await admin.put(`${path}/decision-log/settings`, { data: { outcomeField: 'output', redactedFields: [] } }),
  ).toBeOK();
  await version(admin, `${path}/decisions/${decision.id}`, 'Stability fixture');
  const release = await snapshot(admin, path, 'Stability fixture');
  await stage(admin, path, release.id);

  // Production, because a stability snapshot measures what production decided.
  const requested = await admin.post(`${path}/approvals`, { data: { releaseId: release.id } });
  await expect(requested).toBeOK();
  const approval: ApiSchemas['ApprovalResponse'] = await requested.json();
  let ownerPage = await openPage(reviewer, `/${appLocale}/sign-in`);
  await ownerPage.getByLabel(m.signIn.email, { exact: true }).fill(email);
  await ownerPage.getByLabel(m.signIn.password, { exact: true }).fill(password);
  await ownerPage.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(ownerPage).toHaveURL(new RegExp(`/${appLocale}/?$`));
  await expect(
    await reviewer.request.post(`/api/v1/projects/${project.id}/approvals/${approval.id}/approve`, {
      headers: { 'x-donka-csrf': '1' },
    }),
  ).toBeOK();
  await waitLive(admin, path, 'production', release.id);

  const runtime = origin('DONKA_E2E_PRODUCTION_RUNTIME_URL');
  const token = await issueToken(admin, path, 'production', 'Stability verification');
  async function decide(value: number, region: string) {
    const response = await services.post(`${runtime}/api/projects/${key}/evaluate/score`, {
      headers: { 'X-Access-Token': token.token, 'X-Donka-Reference': unique('stability') },
      data: { context: { input: value, region } },
    });
    expect(response.status()).toBe(200);
    const id = response.headers()['x-decision-id'];
    expect(id, 'the production Runtime names the record it queued').toMatch(/^[0-9a-f-]{36}$/);
    return id;
  }
  async function received(total: number) {
    await expect
      .poll(
        async () => {
          const response = await admin.get(`${path}/decision-log`, { params: { decisionKey: 'score' } });
          await expect(response).toBeOK();
          return (await response.json()).total;
        },
        { timeout: 60_000 },
      )
      .toBe(total);
  }

  // Wait for the production release to answer before timing anything, so the boundary below
  // separates the two populations rather than the Runtime's own start-up.
  await expect
    .poll(
      async () =>
        (
          await services.post(`${runtime}/api/projects/${key}/evaluate/score`, {
            headers: { 'X-Access-Token': token.token },
            data: { context: { input: 1, region: 'warmup' } },
          })
        ).status(),
      { timeout: 60_000 },
    )
    .toBe(200);
  await received(1);

  // The reference population: six decisions in the north, scoring 20.
  const reference = [];
  for (let index = 0; index < 6; index += 1) reference.push(await decide(10, 'north'));
  await received(1 + reference.length);

  // Records carry the instant the Runtime decided them, and nothing can backdate one, so the
  // two periods are separated in real time: everything already sent is the reference, and the
  // boundary is taken after the log has them all.
  const boundary = new Date();
  await new Promise((resolve) => setTimeout(resolve, 1_100));

  // The recent population: the region moved south and the scores doubled.
  const recent = [];
  for (let index = 0; index < 6; index += 1) recent.push(await decide(30, 'south'));
  // One lone northern case, below the minimum bin, so suppression has something to hide.
  recent.push(await decide(10, 'north'));
  await received(1 + reference.length + recent.length);

  // The two periods are seconds apart, which the page's date fields cannot express, so the
  // shifted snapshot is taken through the API with the exact instants. The page then renders
  // that real snapshot, which is what the assertions below read.
  const taken = await admin.post(`${path}/stability`, {
    data: {
      decisionKey: 'score',
      referenceFrom: new Date(boundary.getTime() - 3_600_000).toISOString(),
      referenceUntil: boundary.toISOString(),
      recentFrom: boundary.toISOString(),
      recentUntil: new Date(boundary.getTime() + 3_600_000).toISOString(),
      outputField: 'output',
      outputBins: [40],
      inputFields: ['region'],
    },
  });
  await expect(taken).toBeOK();
  const snapshotTaken: {
    worstBand: string;
    recordsRead: number;
    mix: { week: string; shares: Record<string, number> }[];
    shifts: { field: string; index: number | null; band: string | null; suppressedBins: number }[];
  } = await taken.json();
  // The region moved from north to south entirely, so it reads as a breach.
  expect(snapshotTaken.worstBand).toBe('shifted');
  expect(snapshotTaken.recordsRead).toBe(14);
  const region = snapshotTaken.shifts.find((shift) => shift.field === 'region');
  expect(region?.band).toBe('shifted');
  // The lone northern case in the recent period is one record: hidden, and counted.
  expect(region?.suppressedBins).toBe(1);
  const output = snapshotTaken.shifts.find((shift) => shift.field === 'output');
  expect(output?.index, 'the score moved across its bin edge').toBeGreaterThan(0);
  // Each week's shares divide that week's records, so they sum to one.
  for (const week of snapshotTaken.mix) {
    const total = Object.values(week.shares).reduce((sum, share) => sum + share, 0);
    expect(total).toBeCloseTo(1, 6);
  }

  const page = await openPage(context, `/${appLocale}/projects/stability?p=${key}&decision=score`);
  await expect(page.getByText(m.stability.limitation, { exact: true })).toBeVisible();

  const result = page.getByRole('region', { name: m.stability.result, exact: true });
  await expect(result).toBeVisible({ timeout: 60_000 });
  // The thresholds and the bin floor a snapshot was measured against are shown with it, so a
  // reader is never left guessing which configuration produced a reading.
  await expect(result.getByText(/0\.1/)).toBeVisible();
  await expect(result.getByText(m.stability.band.shifted, { exact: true }).first()).toBeVisible();

  // The mix divides each week by outcome: both scores appear as columns.
  const mix = result.getByLabel(m.stability.mix, { exact: true });
  await expect(mix.getByRole('columnheader', { name: '20', exact: true })).toBeVisible();
  await expect(mix.getByRole('columnheader', { name: '60', exact: true })).toBeVisible();

  const shifts = result.getByLabel(m.stability.shifts, { exact: true });
  await expect(shifts.getByRole('rowheader', { name: 'output', exact: true })).toBeVisible();
  await expect(shifts.getByRole('rowheader', { name: 'region', exact: true })).toBeVisible();
  // One record could point at one person, so the bin is hidden and counted rather than shown.
  await expect(shifts.getByText(m.stability.suppressedBins.replace('{count}', '1'), { exact: true })).toBeVisible();

  await testInfo.attach('stability-snapshot', {
    body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
    contentType: 'image/png',
  });

  // The page never scrolls sideways, including at 390 px: each table scrolls inside its own area.
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);

  // The form takes a snapshot of its own, over a period with nothing in it, so a measure that
  // cannot be taken says so instead of showing a number that would read as stable.
  const iso = (date: Date) => date.toISOString().slice(0, 10);
  const soon = new Date(boundary.getTime() + 3 * 24 * 3600 * 1000);
  const later = new Date(boundary.getTime() + 4 * 24 * 3600 * 1000);
  await page.getByLabel(m.stability.decision, { exact: true }).fill('score');
  await page.getByLabel(m.stability.outputField, { exact: true }).fill('output');
  await page.getByLabel(m.stability.outputBins, { exact: true }).fill('40');
  await page.getByLabel(m.stability.inputFields, { exact: true }).fill('region');
  await page.getByLabel(m.stability.referenceFrom, { exact: true }).fill(iso(soon));
  await page.getByLabel(m.stability.referenceUntil, { exact: true }).fill(iso(soon));
  await page.getByLabel(m.stability.recentFrom, { exact: true }).fill(iso(later));
  await page.getByLabel(m.stability.recentUntil, { exact: true }).fill(iso(later));
  await page.getByRole('button', { name: m.stability.run, exact: true }).click();
  await expect(result.getByText(m.stability.unmeasurable.tooFewRecords, { exact: true }).first()).toBeVisible({
    timeout: 60_000,
  });
  // Snapshots are kept, so the first one is now listed beneath the new one.
  await expect(page.getByText(m.stability.earlier, { exact: true })).toBeVisible();

  // Reading the records is accounted for: the snapshot is audited, and the aggregate is one
  // entry per period rather than one per record opened.
  const audit = await admin.get(`${path}/audit`, { params: { action: 'stability.snapshot_taken' } });
  await expect(audit).toBeOK();
  expect((await audit.json()).total).toBe(2);
  const aggregated = await admin.get(`${path}/audit`, { params: { action: 'decision_records.aggregated' } });
  await expect(aggregated).toBeOK();
  // Two snapshots, two periods each: four entries, not one per record read.
  expect((await aggregated.json()).total).toBe(4);

  // A project's members see only their own project, and the page is the owner's.
  ownerPage = await openPage(reviewer, `/${appLocale}/projects/stability?p=${key}&decision=score`, ownerPage);
  await expect(ownerPage.getByText(m.stability.title, { exact: true })).toBeVisible();
});
