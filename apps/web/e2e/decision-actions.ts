import type { Page } from '@playwright/test';
import type { ApiSchemas } from '../src/components/shared/api';
import { expect } from './fixtures';

/** Save through the form and return the real version, including its scenario results. */
export async function saveVersion(
  page: Page,
  decisionPath: string,
  message: string,
  labels: { save: string; message: string },
): Promise<ApiSchemas['SavedVersionResponse']> {
  await page.getByRole('button', { name: labels.save, exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(labels.message, { exact: true }).fill(message);
  const saved = page.waitForResponse(
    (reply) => reply.url().endsWith(`${decisionPath}/versions`) && reply.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: labels.save, exact: true }).click();
  const response = await saved;
  expect(response.ok()).toBe(true);
  await expect(dialog).toBeHidden();
  return response.json();
}
