import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, newUser, totp, unique } from './fixtures';
import { openPage } from './navigation';

type Installation = ApiSchemas['InstallationResponse'];

/**
 * The first-run assistant (DNK-74). The stack starts in its first run, so the first project to
 * get here walks the whole assistant as a new administrator: second step, organisation, packs.
 * Every project then checks the set-up installation in its locale, theme and width: the
 * assistant is gone, the organisation names Studio, and administrators rename it in Settings.
 */
test('an administrator sets Donka up in the browser; the organisation then names Studio', async ({
  admin,
  services,
  reviewer,
  messages: m,
  appLocale,
}) => {
  const installation = async () => (await (await admin.get('/api/v1/installation')).json()) as Installation;
  const user = await newUser(admin, services, appLocale, true);
  let page = await openPage(reviewer, `/${appLocale}/sign-in`);
  await page.getByLabel(m.signIn.email, { exact: true }).fill(user.email);
  await page.getByLabel(m.signIn.password, { exact: true }).fill(user.password);
  await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/${appLocale}/?$`));

  if ((await installation()).firstRun) {
    // Projects the catalogue would collide with: a pack already imported is shown, not offered.
    const packs = ((await (await admin.get('/api/v1/packs')).json()) as ApiSchemas['PackListResponse']).items;
    const taken = new Set(
      ((await (await admin.get('/api/v1/projects?limit=100')).json()) as { items: { key: string }[] }).items.map(
        (project) => project.key,
      ),
    );
    const fresh = packs.find((pack) => !taken.has(pack.key));

    await page.getByRole('link', { name: m.installation.noticeAction, exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/${appLocale}/first-run/?$`));
    await expect(page.getByRole('heading', { name: m.installation.title, exact: true })).toBeVisible();

    // 1. The second step, required: QR code, a first code, the recovery codes saved.
    const key = await page.getByLabel(m.secondStep.keyLabel, { exact: true }).inputValue();
    await page.getByLabel(m.secondStep.code, { exact: true }).fill(totp(key));
    await page.getByRole('button', { name: m.secondStep.turnOn, exact: true }).click();
    await expect(page.getByRole('list', { name: m.secondStep.codesLabel }).getByRole('listitem')).toHaveCount(10);
    await page.getByLabel(m.secondStep.codesSaved, { exact: true }).check();
    await page.getByRole('button', { name: m.installation.continue, exact: true }).click();

    // 2. The organisation.
    const organisation = unique('E2E Microfinance');
    await page.getByLabel(new RegExp(`^${m.installation.organisationName}`)).fill(organisation);
    await page.getByRole('button', { name: m.installation.continue, exact: true }).click();

    // 3. Packs: none ticked; the chosen one becomes a project owned by this administrator.
    const boxes = page.getByRole('list', { name: m.packs.catalogue }).getByRole('checkbox');
    await expect(boxes).toHaveCount(packs.length);
    for (const box of await boxes.all()) {
      if (await box.isEnabled()) await expect(box).not.toBeChecked();
    }
    if (fresh) {
      const name = appLocale === 'fr' && fresh.name.fr ? fresh.name.fr : fresh.name.en;
      await page.getByRole('checkbox', { name: new RegExp(name) }).check();
    }
    await page.getByRole('button', { name: m.installation.finish, exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/${appLocale}/?$`));
    expect(await installation()).toEqual({ firstRun: false, organisation });
    if (fresh) {
      const project = await reviewer.request.get(`/api/v1/projects/by-key/${fresh.key}`);
      await expect(project).toBeOK();
      expect((await project.json()).role).toBe('owner');
    }
  }

  // Set up: the assistant is gone, Studio names the organisation, administrators rename it.
  const { organisation } = await installation();
  expect(organisation).toBeTruthy();
  page = await openPage(reviewer, `/${appLocale}/first-run`, page);
  await expect(page.getByText(m.installation.doneTitle, { exact: true })).toBeVisible();
  page = await openPage(reviewer, `/${appLocale}/settings`, page);
  await expect(page).toHaveTitle(new RegExp(`${organisation}$`));
  const field = page.getByLabel(new RegExp(`^${m.installation.organisationName}`));
  await expect(field).toHaveValue(organisation ?? '');
  await field.fill(`${organisation} `);
  await page.getByRole('button', { name: m.common.save, exact: true }).click();
  await expect(page.getByText(m.installation.renamed, { exact: true })).toBeVisible();
  // Saved trimmed: the same name, so nothing changed.
  expect((await installation()).organisation).toBe(organisation);
});
