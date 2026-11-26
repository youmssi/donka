import { test, expect, passwordLink, unique } from './fixtures';
import { openPage } from './navigation';

test.use({ storageState: { cookies: [], origins: [] } });

test('invalid and replaced invitation links are refused without changing the account', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}) => {
  const email = `${unique('invalid-link')}@donka.local`;
  const password = 'disposable invitation security passphrase';
  const invitation = { email, locale: appLocale, isAdmin: false };
  await expect(await admin.post('/api/v1/users/invitations', { data: invitation })).toBeOK();
  const old = await passwordLink(services, email);
  await expect(await admin.post('/api/v1/users/invitations', { data: invitation })).toBeOK();
  const current = await passwordLink(services, email, old);
  let page = await openPage(context, `/${appLocale}/setup-password`);
  await expect(page.getByText(m.errors.INVALID_SETUP_LINK, { exact: true })).toBeVisible();
  for (const token of [unique('forged'), old]) {
    page = await openPage(context, `/${appLocale}/setup-password/?token=${token}`, page);
    await page.getByLabel(m.setupPassword.password, { exact: true }).fill(password);
    await page.getByLabel(m.setupPassword.confirm, { exact: true }).fill(password);
    const reply = page.waitForResponse((response) => response.url().endsWith('/auth/password-setup'));
    await page.getByRole('button', { name: m.setupPassword.submit, exact: true }).click();
    expect((await reply).status()).toBe(400);
    await expect(page.getByText(m.errors.INVALID_SETUP_LINK, { exact: true })).toBeVisible();
    await expect(page.getByRole('link', { name: m.setupPassword.requestNewLink, exact: true })).toBeVisible();
  }
  page = await openPage(context, `/${appLocale}/setup-password/?token=${current}`, page);
  await page.getByLabel(m.setupPassword.password, { exact: true }).fill(password);
  await page.getByLabel(m.setupPassword.confirm, { exact: true }).fill(password);
  await page.getByRole('button', { name: m.setupPassword.submit, exact: true }).click();
  await expect(page.getByText(m.setupPassword.doneTitle, { exact: true })).toBeVisible();
});

test('unknown accounts, wrong passwords and a locked account share the same visible failure', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}) => {
  const email = `${unique('locked-user')}@donka.local`;
  const password = 'disposable account lockout passphrase';
  await expect(
    await admin.post('/api/v1/users/invitations', {
      data: { email, locale: appLocale, isAdmin: false },
    }),
  ).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', {
      data: { token: await passwordLink(services, email), password },
    }),
  ).toBeOK();
  const page = await openPage(context, `/${appLocale}/sign-in`);
  const submit = page.getByRole('button', { name: m.signIn.submit, exact: true });
  for (const [address, value] of [
    [`${unique('unknown')}@donka.local`, password],
    ...Array.from({ length: 5 }, () => [email, 'incorrect disposable passphrase']),
    [email, password],
  ]) {
    if (!address || !value) throw new Error('Missing sign-in fixture');
    await page.getByLabel(m.signIn.email, { exact: true }).fill(address);
    await page.getByLabel(m.signIn.password, { exact: true }).fill(value);
    const reply = page.waitForResponse((response) => response.url().endsWith('/auth/sign-in'));
    await submit.click();
    const response = await reply;
    expect(response.status()).toBe(401);
    expect((await response.json()).code).toBe('INVALID_CREDENTIALS');
    await expect(page.getByText(m.errors.INVALID_CREDENTIALS, { exact: true })).toBeVisible();
    await expect(submit).toBeEnabled();
    await expect(page.getByLabel(m.signIn.email, { exact: true })).toHaveValue(address);
  }
  expect((await page.request.get('/api/v1/auth/me')).status()).toBe(401);
});
