import { expect, test } from './fixtures';
import { settings } from './settings';

// Runtime documentation is public and must not need a Studio session.
test.use({ storageState: { cookies: [], origins: [] } });

test('Runtime serves its OpenAPI document and renders the local Swagger UI', async ({ page }) => {
  const { runtime } = settings();
  await expect(await page.request.get(new URL('/api.json', runtime).href)).toBeOK();

  await page.goto(new URL('/api/docs/', runtime).href);
  await expect(page.getByRole('heading', { name: /donka-runtime/i })).toBeVisible();
  await expect(page.getByText('/api/health', { exact: true })).toBeVisible();
});
