import { test, expect } from './fixtures';

test.use({ storageState: { cookies: [], origins: [] } });

test('an invited person signs in through Keycloak and receives an ordinary Studio session', async ({
  page,
  admin,
  appLocale,
  messages: m,
}) => {
  test.skip(process.env.DONKA_E2E_OIDC !== 'true', 'Keycloak is enabled in the install CI stack.');
  const email = 'sso-admin@donka.local';
  const users = await admin.get('/api/v1/users');
  await expect(users).toBeOK();
  const accountExists = (await users.json()).items.some((account: { email: string }) => account.email === email);
  if (!accountExists) {
    await expect(
      await admin.post('/api/v1/users/invitations', {
        data: { email, locale: appLocale, isAdmin: false },
      }),
    ).toBeOK();
  }

  await page.goto(`/${appLocale}/sign-in`);
  await page.getByRole('link', { name: m.signIn.organisationSignIn, exact: true }).click();
  await expect(page).toHaveURL(/keycloak:8180\/realms\/donka\/protocol\/openid-connect\/auth/);
  await page.getByLabel('Username or email').fill(email);
  await page.getByLabel('Password').fill('feature sso passphrase');
  await page.getByRole('button', { name: 'Sign In', exact: true }).click();

  await expect(page).toHaveURL(new RegExp(`/${appLocale}/?$`));
  const session = await page.request.get(`${process.env.DONKA_E2E_BASE_URL}/api/v1/auth/me`);
  await expect(session).toBeOK();
  expect(await session.json()).toMatchObject({ email, isAdmin: true });
  const signedOut = await page.request.post(`${process.env.DONKA_E2E_BASE_URL}/api/v1/auth/sign-out`, {
    headers: { 'x-donka-csrf': '1' },
  });
  await expect(signedOut).toBeOK();
  expect((await page.request.get(`${process.env.DONKA_E2E_BASE_URL}/api/v1/auth/me`)).status()).toBe(401);
});
