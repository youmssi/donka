import { test, expect } from './fixtures';
import { openPage } from './navigation';
import { recordProject, recordViewer } from './record-fixtures';
import { origin } from './settings';

interface CapturedCall {
  body: { model: string; messages: { role: string; content: string }[] };
  authenticated: boolean;
  path: string;
}

test('explanations redact owner and contract fields, keep the viewer language and surface provider errors', async ({
  explaining,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const fixture = await recordProject(admin, services);
  const { key, path } = fixture;
  const endpoint = `${origin('DONKA_E2E_EXPLAIN_SERVICE_URL')}/cases/${key}`;
  let signedIn = false;
  try {
    let page = await openPage(explaining, `/${appLocale}/projects/settings?p=${key}`);
    const field = page.getByLabel(m.decisionLog.redactedFields, { exact: true });
    await field.fill('invalid path');
    const form = page.locator('form').filter({ has: field });
    await form.getByRole('button', { name: m.common.save, exact: true }).click();
    await expect(form.getByText(m.validation.fieldPathLines, { exact: true })).toBeVisible();
    await field.fill('applicant.name');
    await form.getByRole('button', { name: m.common.save, exact: true }).click();
    await expect
      .poll(async () => (await (await admin.get(`${path}/decision-log/settings`)).json()).redactedFields)
      .toEqual(['applicant.name']);
    const record = await fixture.emit();
    await fixture.received(record.reference);
    const before = await (await admin.get(`${path}/decisions/${fixture.decision.id}`)).json();
    const viewer = await recordViewer(admin, services, path, appLocale);
    await explaining.clearCookies();
    page = await openPage(explaining, `/${appLocale}/sign-in`, page);
    await page.getByLabel(m.signIn.email, { exact: true }).fill(viewer.email);
    await page.getByLabel(m.signIn.password, { exact: true }).fill(viewer.password);
    await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/${appLocale}/?$`));
    signedIn = true;
    page = await openPage(explaining, `/${appLocale}/projects/decision-record?p=${key}&r=${record.id}`, page);
    const expected =
      appLocale === 'fr'
        ? 'Le score du service est 712. La règle a conservé ce résultat.'
        : 'The service score is 712. The rule preserved that result.';
    const explain = async () => page.getByRole('button', { name: m.decisionLog.explain, exact: true }).click();
    await explain();
    await expect(page.getByText(expected, { exact: true })).toBeVisible();
    async function captured(): Promise<CapturedCall[]> {
      const response = await services.get(endpoint);
      await expect(response).toBeOK();
      return (await response.json()).calls;
    }
    let calls = await captured();
    expect(calls).toHaveLength(1);
    const call = calls[0];
    if (!call) throw new Error('The endpoint did not capture an explanation');
    expect(call).toMatchObject({ authenticated: true, path: '/v1/messages', body: { model: 'feature-explain-model' } });
    expect(call.body).not.toHaveProperty('tools');
    const outbound = JSON.stringify(call.body);
    expect(outbound).not.toContain(fixture.input.applicant.nationalId);
    expect(outbound).not.toContain(fixture.input.applicant.name);
    expect(outbound).not.toContain('nationalId');
    expect(outbound).not.toContain('feature-explain-secret');
    expect(outbound).toContain(key);
    expect(outbound).toContain('712');
    expect(call.body.messages.at(-1)?.content).toMatch(
      new RegExp(`Write the explanation in ${appLocale === 'fr' ? 'French' : 'English'}\\.$`),
    );
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.evaluate(() => window.scrollTo(0, 0));
    await testInfo.attach('record-explanation', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
    // A fresh page does not retain the explanation; requesting it again calls the endpoint.
    page = await openPage(explaining, page.url(), page);
    await expect(page.getByRole('button', { name: m.decisionLog.explain, exact: true })).toBeVisible();
    await expect(page.getByText(expected, { exact: true })).toHaveCount(0);
    await explain();
    await expect(page.getByText(expected, { exact: true })).toBeVisible();
    expect(await captured()).toHaveLength(2);
    for (const [mode, code] of [
      ['unavailable', 'EXPLAIN_UNAVAILABLE'],
      ['declined', 'EXPLAIN_DECLINED'],
      ['unreadable', 'EXPLAIN_UNAVAILABLE'],
    ] as const) {
      await expect(await services.put(endpoint, { data: { mode } })).toBeOK();
      await explain();
      await expect(page.getByText(m.errors[code], { exact: true })).toBeVisible();
      await expect(page.getByRole('button', { name: m.decisionLog.explain, exact: true })).toBeEnabled();
    }
    calls = await captured();
    expect(calls).toHaveLength(5);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.evaluate(() => window.scrollTo(0, 0));
    await testInfo.attach('record-explanation-error', {
      body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
      contentType: 'image/png',
    });
    expect(await (await admin.get(`${path}/decisions/${fixture.decision.id}`)).json()).toEqual(before);
    const audit = await admin.get(`${path}/audit`, { params: { action: 'decision_record.explained' } });
    await expect(audit).toBeOK();
    const events = await audit.json();
    expect(events.total).toBe(5);
    for (const event of events.items) expect(event.actor.id).toBe(viewer.userId);
    expect(await fixture.calls()).toHaveLength(1);
  } finally {
    if (signedIn)
      await expect(
        await explaining.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } }),
      ).toBeOK();
    await fixture.cleanup();
  }
});
