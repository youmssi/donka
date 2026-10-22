import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, newUser, unique } from './fixtures';
import { openPage } from './navigation';
import { importCatalogue } from './pack-actions';
import { settings } from './settings';

test('a newcomer follows real progress to Runtime; tours can be skipped, replayed and remembered after signing in again', async ({
  admin,
  services,
  reviewer,
  messages: m,
  appLocale,
}, testInfo) => {
  const user = await newUser(admin, services, appLocale, true);
  const othersTours = await (await admin.get('/api/v1/me/tours')).json();
  const graphBundle = Promise.withResolvers<void>();
  let holdingGraph = false;
  await reviewer.route('**/_next/static/**/*.js', async (route) => {
    const response = await route.fetch();
    if (!holdingGraph && (await response.text()).includes('grl-dg__simulator__section__bar--request')) {
      holdingGraph = true;
      await graphBundle.promise;
    }
    await route.fulfill({ response });
  });
  let page = await openPage(reviewer, `/${appLocale}/sign-in`);
  async function signIn() {
    await page.getByLabel(m.signIn.email, { exact: true }).fill(user.email);
    await page.getByLabel(m.signIn.password, { exact: true }).fill(user.password);
    await page.getByRole('button', { name: m.signIn.submit, exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/${appLocale}/?$`));
  }
  await signIn();
  let token: ApiSchemas['IssuedTokenResponse'] | undefined;
  let path = '';
  try {
    const checklist = () => page.locator('[aria-labelledby="get-started-title"]');
    async function progress(done: boolean[]) {
      const response = await reviewer.request.get('/api/v1/onboarding');
      await expect(response).toBeOK();
      const state: ApiSchemas['OnboardingResponse'] = await response.json();
      expect(state.steps.map((step) => step.done)).toEqual(done);
      if (done.every(Boolean)) await expect(checklist()).toHaveCount(0);
      else {
        await expect(checklist()).toBeVisible();
        await expect(checklist().getByRole('listitem')).toHaveCount(5);
        await expect(checklist().getByText(m.onboarding.done, { exact: true })).toHaveCount(
          done.filter(Boolean).length,
        );
      }
      return state;
    }
    await progress([false, false, false, false, false]);
    expect((await (await reviewer.request.get('/api/v1/me/tours')).json()).seen).toEqual([]);
    await checklist().getByRole('button', { name: m.packs.fromPack, exact: true }).click();
    const key = unique('onboarding');
    const catalogue: ApiSchemas['PackListResponse'] = await (await admin.get('/api/v1/packs')).json();
    const pack = catalogue.items.find((item) => item.key === 'mobile-money-limits');
    if (!pack) throw new Error('The onboarding starter pack is missing');
    const projectName = 'My first decision service';
    const imported = await importCatalogue(
      page,
      appLocale === 'fr' ? (pack.name.fr ?? pack.name.en) : pack.name.en,
      key,
      projectName,
      m,
    );
    path = `/api/v1/projects/${imported.project.id}`;
    page = await openPage(reviewer, `/${appLocale}`, page);
    await progress([true, false, true, false, false]);
    await checklist()
      .getByRole('link', { name: m.onboarding.simulation.action.replace('{project}', projectName), exact: true })
      .click();
    await page.getByRole('link', { name: 'transaction-limits', exact: true }).click();
    const title = () => page.locator('.driver-popover-title');
    try {
      await expect.poll(() => holdingGraph).toBe(true);
      await expect(page.locator('[data-tour="editor-status"]')).toBeVisible();
      await expect(page.getByTestId('rf__wrapper')).toHaveCount(0);
      await expect(title()).toHaveCount(0);
      await page.getByRole('button', { name: m.help.label, exact: true }).click();
      await expect(page.getByRole('menuitem', { name: m.help.noTour, exact: true })).toBeDisabled();
      await page.keyboard.press('Escape');
      await expect(title()).toHaveCount(0);
    } finally {
      graphBundle.resolve();
      await reviewer.unrouteAll({ behavior: 'wait' });
    }
    await expect(page.getByTestId('rf__wrapper')).toBeVisible();
    await expect(title()).toHaveText(m.tours.editor.graph.title);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await testInfo.attach('onboarding-tour', {
      body: await page.screenshot({ animations: 'disabled' }),
      contentType: 'image/png',
    });
    await page.getByRole('button', { name: m.tour.skip, exact: true }).click();
    await expect
      .poll(async () => (await (await reviewer.request.get('/api/v1/me/tours')).json()).seen)
      .toEqual(['editor']);
    // Tour completion is unrelated to checklist progress.
    expect(
      (await (await reviewer.request.get('/api/v1/onboarding')).json()).steps.map(
        (step: { done: boolean }) => step.done,
      ),
    ).toEqual([true, false, true, false, false]);
    // Keyboard navigation follows the supported reduced-motion preference, after its first rendered frame.
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.getByRole('button', { name: m.help.label, exact: true }).click();
    await page.getByRole('menuitem', { name: m.help.tour, exact: true }).click();
    await expect(title()).toHaveText(m.tours.editor.graph.title);
    await page.evaluate(
      () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))),
    );
    await page.keyboard.press('ArrowRight');
    await expect(title()).toHaveText(m.tours.editor.status.title);
    await page.keyboard.press('Escape');
    await expect(title()).toHaveCount(0);

    const decisions: ApiSchemas['DecisionListResponse'] = await (
      await reviewer.request.get(`${path}/decisions`)
    ).json();
    const decision = decisions.items[0];
    if (!decision) throw new Error('The imported decision is missing');
    const scenarios: ApiSchemas['ScenarioListResponse'] = await (
      await reviewer.request.get(`${path}/test-scenarios`)
    ).json();
    const sample = scenarios.items[0];
    if (!sample) throw new Error('The starter pack has no scenario input');
    await page.locator('.grl-dg__aside__side-bar__bottom button').click();
    const input = page.locator('.grl-dg__simulator textarea').first();
    await expect(input).toBeEditable();
    await input.focus();
    await input.press('ControlOrMeta+A');
    await page.keyboard.insertText(JSON.stringify(sample.input));
    const simulating = page.waitForResponse((response) =>
      response.url().endsWith(`/decisions/${decision.id}/simulate`),
    );
    await page.getByRole('button', { name: 'Run' }).click();
    const simulated = await simulating;
    expect(simulated.ok()).toBe(true);
    expect((await simulated.json()).result).toMatchObject(sample.expected);
    const outcome = (sample.expected as Record<string, unknown>).decision;
    expect(typeof outcome).toBe('string');
    await expect(page.locator('.grl-dg__simulator__response .view-lines')).toContainText(String(outcome));
    page = await openPage(reviewer, `/${appLocale}`, page);
    await progress([true, true, true, false, false]);
    await checklist()
      .getByRole('link', { name: m.onboarding.staging.action.replace('{project}', projectName), exact: true })
      .click();
    await expect(title()).toHaveText(m.tours.releases.new.title);
    for (const expected of [
      m.tours.releases.list.title,
      m.tours.releases.deploy.title,
      m.tours.releases.approval.title,
    ]) {
      await page.getByRole('button', { name: m.tour.next, exact: true }).click();
      await expect(title()).toHaveText(expected);
    }
    await page.getByRole('button', { name: m.tour.done, exact: true }).click();
    await expect
      .poll(async () => (await (await reviewer.request.get('/api/v1/me/tours')).json()).seen.sort())
      .toEqual(['editor', 'releases']);
    const preview: ApiSchemas['ReleasePreviewResponse'] = await (
      await reviewer.request.get(`${path}/releases/preview`)
    ).json();
    await page.getByRole('button', { name: m.releases.create, exact: true }).first().click();
    let dialog = page.getByRole('dialog');
    await dialog.getByLabel(m.releases.notes, { exact: true }).fill('First service from onboarding');
    const creating = page.waitForResponse(
      (response) => response.url().endsWith(`${path}/releases`) && response.request().method() === 'POST',
    );
    await dialog
      .getByRole('button', { name: m.releases.createVersion.replace('{version}', preview.next.minor), exact: true })
      .click();
    const release: ApiSchemas['ReleaseResponse'] = await (await creating).json();
    page = await openPage(reviewer, `/${appLocale}/projects/environments?p=${key}`, page);
    await expect(title()).toHaveText(m.tours.environments.staging.title);
    await page.keyboard.press('Escape');
    const staging = page.getByRole('region', { name: m.environments.staging, exact: true });
    await staging.getByRole('button', { name: m.environments.deploy, exact: true }).click();
    await expect
      .poll(
        async () => {
          const environments: ApiSchemas['EnvironmentListResponse'] = await (
            await reviewer.request.get(`${path}/environments`)
          ).json();
          return environments.items.find((environment) => environment.environment === 'staging')?.live?.releaseId;
        },
        { timeout: 60_000 },
      )
      .toBe(release.id);
    page = await openPage(reviewer, `/${appLocale}`, page);
    await progress([true, true, true, true, false]);
    await checklist()
      .getByRole('link', { name: m.onboarding.token.action.replace('{project}', projectName), exact: true })
      .click();
    await page
      .getByRole('region', { name: m.environments.staging, exact: true })
      .getByRole('button', { name: m.tokens.create, exact: true })
      .click();
    dialog = page.getByRole('dialog');
    await dialog.getByLabel(m.tokens.name, { exact: true }).fill('My first caller');
    const issuing = page.waitForResponse(
      (response) =>
        response.url().endsWith(`${path}/environments/staging/tokens`) && response.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: m.tokens.create, exact: true }).click();
    token = await (await issuing).json();
    await dialog.getByRole('button', { name: m.tokens.done, exact: true }).click();
    const caller = token;
    if (!caller) throw new Error('No Runtime token was issued');
    const evaluate = () =>
      services.post(`${settings().runtime}/api/projects/${key}/evaluate/${decision.key}`, {
        headers: { 'X-Access-Token': caller.token },
        data: { context: sample.input },
      });
    await expect.poll(async () => (await evaluate()).status(), { timeout: 60_000 }).toBe(200);
    expect((await (await evaluate()).json()).result).toMatchObject(sample.expected);
    page = await openPage(reviewer, `/${appLocale}`, page);
    expect((await progress([true, true, true, true, true])).complete).toBe(true);
    await testInfo.attach('onboarding-complete', {
      body: await page.screenshot({ animations: 'disabled' }),
      contentType: 'image/png',
    });
    const seen = ['editor', 'environments', 'releases'];
    expect((await (await reviewer.request.get('/api/v1/me/tours')).json()).seen.sort()).toEqual(seen);
    expect(await (await admin.get('/api/v1/me/tours')).json()).toEqual(othersTours);
    await expect(await reviewer.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
    page = await openPage(reviewer, `/${appLocale}/sign-in`, page);
    await signIn();
    await progress([true, true, true, true, true]);
    expect((await (await reviewer.request.get('/api/v1/me/tours')).json()).seen.sort()).toEqual(seen);
    page = await openPage(reviewer, `/${appLocale}/projects/decision?p=${key}&d=${decision.key}`, page);
    await expect(page.getByTestId('rf__wrapper')).toBeVisible();
    await expect(title()).toHaveCount(0);
    await page.getByRole('button', { name: m.help.label, exact: true }).click();
    await page.getByRole('menuitem', { name: m.help.tour, exact: true }).click();
    await expect(title()).toHaveText(m.tours.editor.graph.title);
    await page.keyboard.press('Escape');
  } finally {
    graphBundle.resolve();
    await reviewer.unrouteAll({ behavior: 'wait' });
    if (token)
      await expect(
        await reviewer.request.delete(`${path}/environments/staging/tokens/${token.id}`, {
          headers: { 'x-donka-csrf': '1' },
        }),
      ).toBeOK();
    await expect(await reviewer.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
  }
});
