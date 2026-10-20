import type { ApiSchemas } from '../src/components/shared/api';
import { createTranslator } from 'next-intl';
import { test, expect, unique } from './fixtures';
import { openPage } from './navigation';
import { saveVersion } from './decision-actions';

test('graph authoring, simulation, versions and scenarios persist through the live editor', async ({
  context,
  admin,
  messages: m,
  appLocale,
}, testInfo) => {
  const validation = createTranslator({ locale: appLocale, messages: m.validation });
  await expect(await admin.put('/api/v1/me/tours/editor')).toBeOK();
  const key = unique('editor');
  const response = await admin.post('/api/v1/projects', {
    data: { key, name: 'Editor verification', description: '' },
  });
  await expect(response).toBeOK();
  const project: ApiSchemas['ProjectResponse'] = await response.json();
  const path = `/api/v1/projects/${project.id}`;
  let page = await openPage(context, `/${appLocale}/projects/decisions?p=${key}`);
  await page.getByRole('button', { name: m.decisions.create, exact: true }).first().click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.decisions.key, { exact: true }).fill('calculate');
  await dialog.getByRole('button', { name: m.decisions.create, exact: true }).click();
  const canvas = page.getByTestId('rf__wrapper');
  await expect(canvas).toBeVisible();
  const palette = page.locator('.draggable-component');
  const componentNames = await palette.allTextContents();
  const components = ['Request', 'Expression', 'Response'].map((name) =>
    componentNames.findIndex((text) => text.includes(name)),
  );
  expect(components.every((index) => index >= 0)).toBe(true);
  // Collapse the palette with its own control so authoring also fits the 390 px canvas.
  await page.locator('.grl-dg__aside__menu__heading button').click();
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error('The graph canvas has no layout');
  for (const [index, component] of components.entries()) {
    await palette.nth(component).dragTo(canvas, {
      targetPosition: { x: bounds.width / 2, y: 90 + index * 160 },
    });
  }
  await expect(page.locator('.react-flow__node')).toHaveCount(3);
  for (const [from, to] of [
    ['inputNode', 'expressionNode'],
    ['expressionNode', 'outputNode'],
  ]) {
    await page
      .locator(`.react-flow__node-${from} .react-flow__handle.source`)
      .dragTo(page.locator(`.react-flow__node-${to} .react-flow__handle.target`));
  }
  await expect(page.locator('.react-flow__edge')).toHaveCount(2);
  await page.getByRole('button', { name: 'Edit Expression', exact: true }).click();
  await page.getByRole('button', { name: 'Add row' }).click();
  await page.locator('.expression-list-item__key [contenteditable="true"]').fill('output');
  await page.locator('.expression-list-item__value .cm-content').fill('input * 2');
  const decisions = await admin.get(`${path}/decisions`);
  await expect(decisions).toBeOK();
  const list: ApiSchemas['DecisionListResponse'] = await decisions.json();
  const decision = list.items.find((item) => item.key === 'calculate');
  if (!decision) throw new Error('The created decision is missing from the project');
  const decisionPath = `${path}/decisions/${decision.id}`;
  await expect
    .poll(async () => {
      const saved = await admin.get(decisionPath);
      await expect(saved).toBeOK();
      return JSON.stringify((await saved.json()).content);
    })
    .toContain('input * 2');
  page = await openPage(context, page.url(), page);
  await expect(page.locator('.react-flow__node')).toHaveCount(3);
  await expect(page.locator('.react-flow__edge')).toHaveCount(2);
  await page.locator('.grl-dg__aside__side-bar__bottom button').click();
  const request = page.locator('.grl-dg__simulator textarea').first();
  await expect(request).toBeEditable();
  await request.focus();
  await request.press('ControlOrMeta+A');
  await page.keyboard.insertText('{"input":12}');
  const simulated = page.waitForResponse((reply) => reply.url().endsWith(`${decisionPath}/simulate`));
  await page.getByRole('button', { name: 'Run' }).click();
  const simulation = await simulated;
  expect(simulation.ok()).toBe(true);
  expect(await simulation.json()).toMatchObject({ result: { input: 12, output: 24 }, trace: expect.any(Object) });
  const output = page.locator('.grl-dg__simulator__response .view-lines');
  await output.scrollIntoViewIfNeeded();
  await expect(output).toContainText(/\boutput:\s*24/);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await expect(page.getByRole('button', { name: m.scenarios.saveRun, exact: true })).toBeEnabled();
  await page.getByRole('button', { name: m.scenarios.saveRun, exact: true }).click();
  const scenarioDialog = page.getByRole('dialog');
  expect(JSON.parse(await scenarioDialog.getByLabel(m.scenarios.input, { exact: true }).inputValue())).toEqual({
    input: 12,
  });
  expect(JSON.parse(await scenarioDialog.getByLabel(m.scenarios.expected, { exact: true }).inputValue())).toEqual({
    input: 12,
    output: 24,
  });
  await scenarioDialog.getByLabel(m.scenarios.name, { exact: true }).fill('Double input');
  await scenarioDialog.getByRole('button', { name: m.scenarios.create, exact: true }).click();
  await expect(scenarioDialog).toBeHidden();
  const version = await saveVersion(page, decisionPath, 'Double input', m.versions);
  expect(version.tests).toEqual({ passed: 1, failed: 0, errors: 0 });
  await expect(page.getByRole('button', { name: m.versions.save, exact: true })).toBeDisabled();
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect.poll(() => page.evaluate(() => scrollY)).toBe(0);
  await testInfo.attach('editor-simulation', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
  page = await openPage(context, page.url(), page);
  await page.getByRole('button', { name: 'Edit Expression', exact: true }).click();
  const expression = page.locator('.expression-list-item__value .cm-content');
  // CodeMirror keeps its own document: replace through its keyboard commands.
  await expression.click();
  await expression.press('ControlOrMeta+A');
  await page.keyboard.insertText('input * 3');
  const second = await saveVersion(page, decisionPath, 'Triple input', m.versions);
  expect(second.number).toBe(2);
  expect(second.tests).toEqual({ passed: 0, failed: 1, errors: 0 });
  await page.getByRole('button', { name: m.versions.history, exact: true }).click();
  const history = page.getByRole('dialog', { name: m.versions.historyTitle, exact: true });
  const versions = history.getByRole('listitem');
  await expect(versions).toHaveCount(2);
  await expect(versions.first()).toContainText('Triple input');
  await versions
    .first()
    .getByRole('button', {
      name: m.tests.summary.replace('{passed}', '0').replace('{failed}', '1').replace('{errors}', '0'),
      exact: true,
    })
    .click();
  const results = page
    .getByRole('dialog')
    .filter({ has: page.getByRole('heading', { name: m.tests.title.replace('{number}', '2'), exact: true }) });
  const mismatch = results.getByRole('row').filter({ hasText: 'output' });
  await expect(mismatch.getByRole('cell')).toHaveText(['output', '24', '36']);
  await results.getByRole('button', { name: m.common.close, exact: true }).first().click();
  await history
    .getByRole('button', { name: m.versions.compareWithDraft.replace('{number}', '1'), exact: true })
    .click();
  const comparison = page.getByRole('dialog', { name: m.versions.compareTitle, exact: true });
  await expect(comparison.locator('.grl-dn--diff-modified')).toHaveCount(1);
  await comparison.getByRole('combobox', { name: m.versions.to, exact: true }).click();
  await page.getByRole('option', { name: 'v2', exact: true }).click();
  await expect(comparison.locator('.grl-dn--diff-modified')).toHaveCount(1);
  await comparison.getByRole('button', { name: m.versions.close, exact: true }).first().click();
  await history.getByRole('button', { name: m.versions.actionsFor.replace('{number}', '1'), exact: true }).click();
  await page.getByRole('menuitem', { name: m.versions.restore, exact: true }).click();
  let confirm = page.getByRole('alertdialog');
  await expect(confirm).toContainText(m.versions.restoreConfirm.replace('{number}', '3'));
  await confirm.getByRole('button', { name: m.common.cancel, exact: true }).click();
  await history.getByRole('button', { name: m.versions.actionsFor.replace('{number}', '1'), exact: true }).click();
  await page.getByRole('menuitem', { name: m.versions.restore, exact: true }).click();
  confirm = page.getByRole('alertdialog');
  const restored = page.waitForResponse((reply) => reply.url().endsWith(`${decisionPath}/versions/1/restore`));
  await confirm.getByRole('button', { name: m.versions.restore, exact: true }).click();
  const restoreResponse = await restored;
  expect(restoreResponse.ok()).toBe(true);
  const restore: ApiSchemas['RestoreResponse'] = await restoreResponse.json();
  expect(restore.version).toMatchObject({ number: 3, restoredFrom: 1, tests: { passed: 1, failed: 0, errors: 0 } });
  const original = await admin.get(`${decisionPath}/versions/1`);
  await expect(original).toBeOK();
  expect(restore.decision.content).toEqual((await original.json()).content);
  await expect(history.getByRole('listitem')).toHaveCount(3);
  await testInfo.attach('version-history', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
  page = await openPage(context, `/${appLocale}/projects/scenarios?p=${key}`, page);
  await page
    .getByRole('button', { name: m.scenarios.actionsFor.replace('{name}', 'Double input'), exact: true })
    .click();
  await page.getByRole('menuitem', { name: m.scenarios.edit, exact: true }).click();
  const editing = page.getByRole('dialog');
  await editing.getByLabel(m.scenarios.expected, { exact: true }).fill('[]');
  await editing.getByRole('button', { name: m.scenarios.save, exact: true }).click();
  await expect(editing.getByText(validation('jsonObject'), { exact: true })).toBeVisible();
  await editing.getByLabel(m.scenarios.expected, { exact: true }).fill('{"output":999}');
  await editing.getByRole('combobox', { name: m.scenarios.match, exact: true }).click();
  await page.getByRole('option', { name: m.scenarios.exact, exact: true }).click();
  await editing.getByRole('button', { name: m.scenarios.save, exact: true }).click();
  await expect(editing).toBeHidden();
  const updated = await admin.get(`${path}/test-scenarios`);
  await expect(updated).toBeOK();
  const scenarios: ApiSchemas['ScenarioListResponse'] = await updated.json();
  expect(scenarios.items[0]).toMatchObject({ match: 'exact', expected: { output: 999 } });
  await page
    .getByRole('button', { name: m.scenarios.actionsFor.replace('{name}', 'Double input'), exact: true })
    .click();
  await page.getByRole('menuitem', { name: m.scenarios.delete, exact: true }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: m.scenarios.delete, exact: true }).click();
  await expect(page.getByText(m.scenarios.emptyTitle, { exact: true })).toBeVisible();
  // Later scenario changes/deletion cannot rewrite version 2's captured input or result.
  const captured = await admin.get(`${decisionPath}/versions/2/test-results`);
  await expect(captured).toBeOK();
  const capturedResults: ApiSchemas['TestResultListResponse'] = await captured.json();
  expect(capturedResults.items).toHaveLength(1);
  expect(capturedResults.items[0]).toMatchObject({
    input: { input: 12 },
    expected: { input: 12, output: 24 },
    actual: { input: 12, output: 36 },
    status: 'failed',
    match: 'partial',
  });
});
