import type { APIRequestContext } from '@playwright/test';
import { createTranslator } from 'next-intl';
import { test, expect, unique } from '../e2e/fixtures';
import { openPage } from '../e2e/navigation';
import { assertLimited, budget } from './limits';

test.use({ storageState: { cookies: [], origins: [] } });

async function exhaust(api: APIRequestContext, path: string, data: object, count: number): Promise<void> {
  for (let attempt = 0; attempt <= count; attempt++) {
    const reply = await api.post(path, { data, headers: { 'x-donka-csrf': '1' } });
    if (reply.status() === 429) return;
    expect(reply.status()).toBe(path.endsWith('sign-in') ? 401 : 202);
  }
  throw new Error('The running service did not enforce its configured authentication budget.');
}

test('configured sign-in and reset limits return Retry-After and translated guidance in the real UI', async ({
  context,
  messages: m,
  appLocale,
}) => {
  const signInBudget = budget('DONKA_E2E_SIGN_IN_RATE_LIMIT');
  const resetBudget = budget('DONKA_E2E_PASSWORD_RESET_RATE_LIMIT');
  const email = `${unique('rate-probe')}@donka.local`;
  const password = 'disposable rate probe passphrase';
  const errors = createTranslator({ locale: appLocale, messages: m.errors });
  let page = await openPage(context, `/${appLocale}/sign-in`);
  await page.getByLabel(m.signIn.email, { exact: true }).fill(email);
  await page.getByLabel(m.signIn.password, { exact: true }).fill(password);
  await exhaust(context.request, '/api/v1/auth/sign-in', { email, password }, signInBudget);
  let response = page.waitForResponse((reply) => reply.url().endsWith('/auth/sign-in'));
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  let seconds = await assertLimited(await response);
  await expect(page.getByText(errors('RATE_LIMITED', { seconds }), { exact: true })).toBeVisible();
  await expect(page.getByLabel(m.signIn.email, { exact: true })).toHaveValue(email);
  expect((await context.request.get('/api/v1/auth/me')).status()).toBe(401);
  page = await openPage(context, `/${appLocale}/forgot-password`, page);
  await page.getByLabel(m.forgotPassword.email, { exact: true }).fill(email);
  await exhaust(context.request, '/api/v1/auth/password-reset', { email }, resetBudget);
  response = page.waitForResponse((reply) => reply.url().endsWith('/auth/password-reset'));
  await page.getByRole('button', { name: m.forgotPassword.submit, exact: true }).click();
  seconds = await assertLimited(await response);
  await expect(page.getByText(errors('RATE_LIMITED', { seconds }), { exact: true })).toBeVisible();
  await expect(page.getByLabel(m.forgotPassword.email, { exact: true })).toHaveValue(email);
});
