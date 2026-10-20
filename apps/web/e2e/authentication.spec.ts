import { test, expect, passwordLink, unique } from './fixtures';

test.use({ storageState: { cookies: [], origins: [] } });

test('invitation, password setup, independent visibility, sign-in and sign-out', async ({
  page,
  admin,
  services,
  messages: m,
  appLocale,
  theme,
}, testInfo) => {
  const email = `${unique('browser-user')}@donka.local`;
  const password = 'a disposable browser verification phrase';
  await expect(
    await admin.post('/api/v1/users/invitations', {
      data: { email, locale: appLocale, isAdmin: false },
    }),
  ).toBeOK();
  const token = await passwordLink(services, email);
  await page.goto(`/${appLocale}/setup-password/?token=${token}`);
  const first = page.getByLabel(m.setupPassword.password, { exact: true });
  const confirm = page.getByLabel(m.setupPassword.confirm, { exact: true });
  await expect(first).toHaveAttribute('type', 'password');
  await expect(confirm).toHaveAttribute('type', 'password');
  await first.fill(password);
  await confirm.fill(`${password} mismatch`);
  await confirm.blur();
  await expect(page.getByText(m.validation.passwordMismatch, { exact: true })).toBeVisible();
  await page
    .getByRole('button', { name: `${m.common.showPassword}: ${m.setupPassword.password}`, exact: true })
    .click();
  await expect(first).toHaveAttribute('type', 'text');
  await expect(confirm).toHaveAttribute('type', 'password');
  await expect(first).toHaveValue(password);
  await page
    .getByRole('button', { name: `${m.common.hidePassword}: ${m.setupPassword.password}`, exact: true })
    .click();
  await confirm.fill(password);
  await expect(page.getByText(m.validation.passwordMismatch, { exact: true })).toHaveCount(0);
  await page.evaluate(() => document.fonts.ready);
  await testInfo.attach('password-controls', {
    body: await page.screenshot({ fullPage: true, animations: 'disabled' }),
    contentType: 'image/png',
  });
  await page.getByRole('button', { name: m.setupPassword.submit, exact: true }).click();
  await expect(page.getByText(m.setupPassword.doneTitle, { exact: true })).toBeVisible();

  await page.goto(`/${appLocale}/sign-in`);
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(page.locator('[aria-invalid="true"]')).toHaveCount(2);
  await page.getByLabel(m.signIn.email, { exact: true }).fill(email);
  const signInPassword = page.getByLabel(m.signIn.password, { exact: true });
  await signInPassword.fill(password);
  await signInPassword.press('Tab');
  await expect(
    page.getByRole('button', { name: `${m.common.showPassword}: ${m.signIn.password}`, exact: true }),
  ).toBeFocused();
  await page.keyboard.press('Space');
  await expect(signInPassword).toHaveAttribute('type', 'text');
  await page.keyboard.press('Enter');
  await expect(signInPassword).toHaveAttribute('type', 'password');
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(page.getByRole('heading', { name: m.projects.title, exact: true })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', appLocale);
  await expect
    .poll(() => page.locator('html').evaluate((html) => html.classList.contains('dark')))
    .toBe(theme === 'dark');
  await page.evaluate(() => document.fonts.ready);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  if (!(await page.getByRole('button', { name: m.common.account, exact: true }).isVisible())) {
    await page.getByRole('button', { name: m.ui.toggleSidebar, exact: true }).click();
  }
  await page.getByRole('button', { name: m.common.account, exact: true }).click();
  await page.getByRole('menuitem', { name: m.common.signOut, exact: true }).click();
  await expect(page.getByLabel(m.signIn.email, { exact: true })).toBeVisible();
  expect((await page.request.get('/api/v1/auth/me')).status()).toBe(401);
});

test('a password reset link changes the password and is single use', async ({
  page,
  admin,
  services,
  messages: m,
  appLocale,
}) => {
  const email = `${unique('browser-reset')}@donka.local`;
  const initial = 'initial disposable browser passphrase';
  const changed = 'changed disposable browser passphrase';
  await expect(
    await admin.post('/api/v1/users/invitations', {
      data: { email, locale: appLocale, isAdmin: false },
    }),
  ).toBeOK();
  const invitation = await passwordLink(services, email);
  // Fixture setup is API-driven; resetting and using the resulting password are the UI behavior under test.
  await expect(
    await admin.post('/api/v1/auth/password-setup', { data: { token: invitation, password: initial } }),
  ).toBeOK();
  // Static controls must not accept input until their event handlers are ready.
  const scripts = '**/_next/static/**/*.js';
  const hydration = Promise.withResolvers<void>();
  await page.route(scripts, async (route) => {
    await hydration.promise;
    await route.continue();
  });
  const resetEmail = page.getByLabel(m.forgotPassword.email, { exact: true });
  try {
    await page.goto(`/${appLocale}/forgot-password`, { waitUntil: 'commit' });
    await expect(resetEmail).toBeVisible();
    await expect(resetEmail).toBeDisabled();
    await expect(page.getByRole('button', { name: m.forgotPassword.submit, exact: true })).toBeDisabled();
  } finally {
    hydration.resolve();
    await page.unrouteAll({ behavior: 'wait' });
  }
  await expect(resetEmail).toBeEnabled();
  await expect(resetEmail).toBeFocused();
  await resetEmail.fill(email);
  await page.getByRole('button', { name: m.forgotPassword.submit, exact: true }).click();
  const reset = await passwordLink(services, email, invitation);
  await page.goto(`/${appLocale}/setup-password/?token=${reset}`);
  await page.getByLabel(m.setupPassword.password, { exact: true }).fill(changed);
  await page.getByLabel(m.setupPassword.confirm, { exact: true }).fill(changed);
  await page.getByRole('button', { name: m.setupPassword.submit, exact: true }).click();
  await expect(page.getByText(m.setupPassword.doneTitle, { exact: true })).toBeVisible();
  const reused = await admin.post('/api/v1/auth/password-setup', { data: { token: reset, password: initial } });
  expect(reused.status()).toBe(400);
  expect((await reused.json()).code).toBe('INVALID_SETUP_LINK');
  await page.goto(`/${appLocale}/sign-in`);
  await page.getByLabel(m.signIn.email, { exact: true }).fill(email);
  await page.getByLabel(m.signIn.password, { exact: true }).fill(changed);
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(page.getByRole('heading', { name: m.projects.title, exact: true })).toBeVisible();
  await expect(await page.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
});
