import type { Locator, Page } from '@playwright/test';
import type { ApiSchemas } from '../src/components/shared/api';
import { expect, type Messages } from './fixtures';

export async function nameProject(dialog: Locator, name: string, key: string, labels: Messages['projects']) {
  await dialog.getByLabel(labels.name, { exact: true }).fill(name);
  await dialog.getByLabel(labels.key, { exact: true }).fill(key);
}

/** Select and import from the open catalogue dialog; no pack is seeded through the API. */
export async function importCatalogue(page: Page, packName: string, key: string, name: string, m: Messages) {
  const dialog = page.getByRole('dialog', { name: m.packs.importTitle, exact: true });
  await dialog.getByRole('radio', { name: packName }).check();
  await nameProject(dialog, name, key, m.projects);
  const imported = page.waitForResponse(
    (response) => /\/packs\/[^/]+\/import$/.test(response.url()) && response.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: m.packs.import, exact: true }).click();
  const response = await imported;
  expect(response.ok()).toBe(true);
  const result: ApiSchemas['ImportedResponse'] = await response.json();
  await expect(dialog).toBeHidden();
  await expect(page).toHaveURL(new RegExp(`p=${key}`));
  expect(result.tests).toMatchObject({ failed: 0, errors: 0 });
  expect(result.tests.passed).toBeGreaterThan(0);
  return result;
}
