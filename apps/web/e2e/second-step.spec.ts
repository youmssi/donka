import { createHmac } from 'node:crypto';

import { test, expect, newUser } from './fixtures';
import { openPage } from './navigation';

test.use({ storageState: { cookies: [], origins: [] } });

/** RFC 6238 code (SHA-1, six digits, 30 s) for a base32 key, at `time` seconds. */
function totp(key: string, time = Date.now() / 1000): string {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  let bits = '';
  for (const char of key.replace(/=+$/, '')) bits += alphabet.indexOf(char).toString(2).padStart(5, '0');
  const secret = Buffer.from(bits.match(/.{8}/g)?.map((byte) => parseInt(byte, 2)) ?? []);
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(time / 30)));
  const mac = createHmac('sha1', secret).update(counter).digest();
  const offset = (mac.at(-1) ?? 0) & 0xf;
  return String((mac.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).padStart(6, '0');
}

test('a person turns on two-step sign-in, signs in with a recovery code and is reset by an administrator', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}) => {
  const { email, password } = await newUser(admin, services, appLocale);
  let page = await openPage(context, `/${appLocale}/sign-in`);
  await page.getByLabel(m.signIn.email, { exact: true }).fill(email);
  await page.getByLabel(m.signIn.password, { exact: true }).fill(password);
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(page).not.toHaveURL(/sign-in/);

  // Account: turn it on with the key the page shows.
  page = await openPage(context, `/${appLocale}/account`, page);
  await expect(page.getByText(m.secondStep.offText, { exact: true })).toBeVisible();
  await page.getByRole('button', { name: m.secondStep.enable, exact: true }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByRole('img', { name: m.secondStep.qrLabel })).toBeVisible();
  const key = await dialog.getByLabel(m.secondStep.keyLabel, { exact: true }).inputValue();
  expect(key).toMatch(/^[A-Z2-7]{32}$/);
  await dialog.getByLabel(m.secondStep.code, { exact: true }).fill(totp(key));
  await dialog.getByRole('button', { name: m.secondStep.turnOn, exact: true }).click();
  const codes = (
    await dialog.getByRole('list', { name: m.secondStep.codesLabel }).getByRole('listitem').allInnerTexts()
  ).map((code) => code.trim());
  expect(codes).toHaveLength(10);
  const done = dialog.getByRole('button', { name: m.secondStep.done, exact: true });
  await expect(done).toBeDisabled();
  await dialog.getByLabel(m.secondStep.codesSaved, { exact: true }).check();
  await done.click();
  await expect(page.getByText(m.secondStep.onText, { exact: true })).toBeVisible();

  // Sign in again: the password alone opens nothing; a recovery code does, once.
  await context.clearCookies();
  page = await openPage(context, `/${appLocale}/sign-in`, page);
  for (const code of [codes[0], codes[0]]) {
    await page.getByLabel(m.signIn.email, { exact: true }).fill(email);
    await page.getByLabel(m.signIn.password, { exact: true }).fill(password);
    const passwordReply = page.waitForResponse((response) => response.url().endsWith('/auth/sign-in'));
    await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
    expect((await passwordReply).status()).toBe(202);
    expect((await page.request.get('/api/v1/auth/me')).status()).toBe(401);
    await expect(page.getByText(m.secondStep.signInTitle, { exact: true })).toBeVisible();
    await page.getByLabel(m.secondStep.codeOrRecovery, { exact: true }).fill(code ?? '');
    const codeReply = page.waitForResponse((response) => response.url().endsWith('/auth/sign-in/code'));
    await page.getByRole('button', { name: m.secondStep.verify, exact: true }).click();
    if ((await codeReply).ok()) {
      await expect(page).not.toHaveURL(/sign-in/);
      await context.clearCookies();
      page = await openPage(context, `/${appLocale}/sign-in`, page);
    } else {
      await expect(page.getByText(m.errors.INVALID_CODE, { exact: true })).toBeVisible();
    }
  }

  // Lost device: an administrator resets it; the person signs in with the password only.
  const people = await admin.get('/api/v1/users?limit=100');
  await expect(people).toBeOK();
  const person = (await people.json()).items.find((account: { email: string }) => account.email === email);
  expect(person.secondStep).toBe(true);
  await expect(await admin.post(`/api/v1/users/${person.id}/second-step/reset`)).toBeOK();
  await page.reload();
  await page.getByLabel(m.signIn.email, { exact: true }).fill(email);
  await page.getByLabel(m.signIn.password, { exact: true }).fill(password);
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(page).not.toHaveURL(/sign-in/);
});
