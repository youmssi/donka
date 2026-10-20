import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, passwordLink, unique } from './fixtures';
import { openPage } from './navigation';

test('administrators invite, resend and see an activated account through People', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const email = `${unique('people-invite')}@donka.local`;
  const invitationLocale = appLocale === 'en' ? 'fr' : 'en';
  let page = await openPage(context, `/${appLocale}/people`);
  await page.getByRole('button', { name: m.people.invite, exact: true }).first().click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.people.email, { exact: true }).fill(email);
  await dialog.getByLabel(m.people.language, { exact: true }).click();
  await page
    .getByRole('option', { name: invitationLocale === 'fr' ? m.common.french : m.common.english, exact: true })
    .click();
  await dialog.getByLabel(m.people.isAdmin, { exact: true }).check();
  const invitation = page.waitForResponse((response) => response.url().endsWith('/users/invitations'));
  await dialog.getByRole('button', { name: m.people.send, exact: true }).click();
  expect((await invitation).ok()).toBe(true);
  await expect(dialog).toBeHidden();
  const old = await passwordLink(services, email);
  // Accounts sort by email. Locate the public pagination offset independently of UI markup.
  let offset = 0;
  for (;;) {
    const listed = await admin.get(`/api/v1/users?limit=100&offset=${offset}`);
    await expect(listed).toBeOK();
    const accounts: ApiSchemas['AccountListResponse'] = await listed.json();
    const index = accounts.items.findIndex((account) => account.email === email);
    if (index >= 0) {
      offset += index;
      break;
    }
    offset += accounts.items.length;
    expect(offset, 'Invited account exists in the paginated directory').toBeLessThan(accounts.total);
  }
  page = await openPage(context, `/${appLocale}/people?offset=${offset}`, page);
  let row = page.getByRole('row').filter({ hasText: email });
  await expect(row.getByText(m.people.pending, { exact: true })).toBeVisible();
  const resend = row.getByRole('button', { name: m.people.resendFor.replace('{email}', email), exact: true });
  await resend.click();
  const replacement = await passwordLink(services, email, old);
  const obsolete = await admin.post('/api/v1/auth/password-setup', {
    data: { token: old, password: 'obsolete disposable invitation passphrase' },
  });
  expect(obsolete.status()).toBe(400);
  expect((await obsolete.json()).code).toBe('INVALID_SETUP_LINK');
  // Accepting the invitation is covered in authentication.spec; here activation is a fixture.
  await expect(
    await admin.post('/api/v1/auth/password-setup', {
      data: { token: replacement, password: 'activated disposable invitation passphrase' },
    }),
  ).toBeOK();
  page = await openPage(context, `/${appLocale}/people?offset=${offset}`, page);
  row = page.getByRole('row').filter({ hasText: email });
  await expect(row.getByText(m.people.active, { exact: true })).toBeVisible();
  await expect(row.getByRole('button')).toHaveCount(0);
  const accountList = await admin.get('/api/v1/users?limit=100&offset=' + offset);
  await expect(accountList).toBeOK();
  const persisted: ApiSchemas['AccountListResponse'] = await accountList.json();
  expect(persisted.items.find((account) => account.email === email)).toMatchObject({
    isAdmin: true,
    locale: invitationLocale,
    active: true,
  });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await testInfo.attach('people-active', { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
});
