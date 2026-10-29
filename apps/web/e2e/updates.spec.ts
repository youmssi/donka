import { test, expect, newUser } from './fixtures';
import { openPage } from './navigation';

// The live stack's Studio checks a disposable release feed that announces Donka 99.0.0
// (scripts/feature-releases, DONKA_UPDATE_CHECK_URL), so a newer release is always published.
test('administrators learn of a newer Donka release and can hide it; others never see it', async ({
  admin,
  services,
  context,
  reviewer,
  messages: m,
  appLocale,
}) => {
  const response = await admin.get('/api/v1/updates');
  await expect(response).toBeOK();
  const updates = await response.json();
  expect(updates).toMatchObject({ checking: true, newer: { version: '99.0.0' } });

  const title = m.updates.title.replace('{version}', '99.0.0');
  let page = await openPage(context, `/${appLocale}/`);
  await expect(page.getByText(title, { exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: m.updates.notes, exact: true })).toHaveAttribute(
    'href',
    updates.newer.url,
  );
  await page.getByRole('button', { name: m.updates.dismiss, exact: true }).click();
  await expect(page.getByText(title, { exact: true })).toHaveCount(0);
  page = await openPage(context, `/${appLocale}/`, page);
  await expect(page.getByRole('main')).toBeVisible();
  await expect(page.getByText(title, { exact: true })).toHaveCount(0);

  const member = await newUser(admin, services, appLocale);
  let other = await openPage(reviewer, `/${appLocale}/sign-in`);
  await other.getByLabel(m.signIn.email, { exact: true }).fill(member.email);
  await other.getByLabel(m.signIn.password, { exact: true }).fill(member.password);
  await other.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  other = await openPage(reviewer, `/${appLocale}/`, other);
  await expect(other.getByRole('main')).toBeVisible();
  await expect(other.getByText(title, { exact: true })).toHaveCount(0);
  expect((await reviewer.request.get('/api/v1/updates')).status()).toBe(403);
  await expect(await reviewer.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
});
