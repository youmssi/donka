import { test, expect } from './fixtures';
import { openPage } from './navigation';
import { recordProject } from './record-fixtures';

/** A message with its placeholders filled. */
function fill(message: string, values: Record<string, string | number>): string {
  return message.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''));
}

test('an owner answers a person: finds their decisions, exports the report, anonymises after the retention rule, records a contest', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}) => {
  const fixture = await recordProject(admin, services);
  const { key, path, input } = fixture;
  try {
    const first = await fixture.emit();
    const second = await fixture.emit();
    await fixture.received(first.reference);
    await fixture.received(second.reference);

    // A retention rule documented in the settings blocks anonymisation, and says why.
    let page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`);
    const rules = page.locator('form').filter({ has: page.getByLabel(m.retentionRules.ruleDescription) });
    await rules.getByLabel(m.retentionRules.ruleDescription).fill('Loan dispute with the applicant');
    await rules.getByLabel(m.retentionRules.legalReference).fill('Case 2026/118');
    const today = await page.evaluate(() => {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, '0');
      return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
    });
    await rules.getByLabel(m.retentionRules.untilLabel).fill(today);
    await rules.getByRole('button', { name: m.retentionRules.create, exact: true }).click();
    await expect(page.getByText('Loan dispute with the applicant', { exact: true })).toBeVisible();

    // The identifier, written another way, finds both decisions.
    page = await openPage(context, `/${appLocale}/projects/people-requests?p=${key}`, page);
    const identifier = input.applicant.nationalId.toLowerCase().replaceAll('-', ' ');
    await page.getByLabel(m.peopleRequests.identifier, { exact: true }).fill(identifier);
    await page.getByRole('button', { name: m.peopleRequests.search, exact: true }).click();
    await expect(page.getByText(fill(m.peopleRequests.foundMany, { count: 2 }), { exact: true })).toBeVisible();
    expect(page.url()).not.toContain('SYNTHETIC');

    // The access report downloads as a PDF in the person's language.
    await page.getByRole('button', { name: m.peopleRequests.report, exact: true }).click();
    const report = page.getByRole('dialog', { name: m.peopleRequests.reportTitle, exact: true });
    const download = page.waitForEvent('download');
    await report.getByRole('button', { name: m.peopleRequests.download, exact: true }).click();
    expect((await download).suggestedFilename()).toBe('access-report.pdf');
    await expect(report).toBeHidden();

    // Anonymising is refused while the rule runs.
    await page.getByRole('button', { name: m.peopleRequests.anonymise, exact: true }).click();
    const anonymise = page.getByRole('dialog', { name: m.peopleRequests.anonymiseTitle, exact: true });
    await anonymise.getByRole('button', { name: m.peopleRequests.anonymiseConfirm, exact: true }).click();
    await expect(anonymise.getByText('Loan dispute with the applicant', { exact: true })).toBeVisible();
    await anonymise.getByRole('button', { name: m.common.cancel, exact: true }).click();

    // Once the rule ends, the person's details leave both records for good.
    const listed = await (await admin.get(`${path}/retention-rules`)).json();
    await expect(await admin.post(`${path}/retention-rules/${listed.items[0].id}/end`)).toBeOK();
    await page.getByRole('button', { name: m.peopleRequests.anonymise, exact: true }).click();
    await anonymise.getByRole('button', { name: m.peopleRequests.anonymiseConfirm, exact: true }).click();
    await expect(page.getByText(m.peopleRequests.noneTitle, { exact: true })).toBeVisible();

    page = await openPage(context, `/${appLocale}/projects/decision-record?p=${key}&r=${first.id}`, page);
    await expect(page.getByText(m.decisionLog.anonymisedTitle, { exact: true })).toBeVisible();
    await expect(page.getByText(input.applicant.nationalId)).toHaveCount(0);

    // A contest and its review are recorded on the decision.
    await page.getByRole('button', { name: m.contests.open, exact: true }).click();
    const open = page.getByRole('dialog', { name: m.contests.openTitle, exact: true });
    await open.getByLabel(m.contests.reason).fill('My salary was not counted.');
    await open.getByRole('button', { name: m.contests.openSubmit, exact: true }).click();
    await expect(page.getByText('My salary was not counted.', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: m.contests.review, exact: true }).click();
    const review = page.getByRole('dialog', { name: m.contests.reviewTitle, exact: true });
    await review.getByRole('combobox', { name: m.contests.outcome }).click();
    await page.getByRole('option', { name: m.contests.statusRevised, exact: true }).click();
    await review.getByLabel(m.contests.newOutcomeLabel).fill('approve');
    await review.getByLabel(m.contests.reviewText).fill('Salary slips checked.');
    await review.getByRole('button', { name: m.contests.reviewSubmit, exact: true }).click();
    await expect(review).toBeHidden();
    const contests = page.getByRole('list', { name: m.contests.title, exact: true });
    await expect(contests.getByText('Salary slips checked.', { exact: true })).toBeVisible();
    await expect(contests.getByText(m.contests.statusRevised, { exact: true })).toBeVisible();

    // Every step is in the audit log, and no identifier is.
    const audit = await (await admin.get(`${path}/audit`, { params: { limit: 100 } })).json();
    const actions = audit.items.map((event: { action: string }) => event.action);
    for (const action of [
      'retention_rule.created',
      'person.records_searched',
      'person.report_exported',
      'retention_rule.ended',
      'person.records_anonymised',
      'decision_contest.opened',
      'decision_contest.reviewed',
    ]) {
      expect(actions).toContain(action);
    }
    expect(JSON.stringify(audit)).not.toContain(input.applicant.nationalId);
  } finally {
    await fixture.cleanup();
  }
});
