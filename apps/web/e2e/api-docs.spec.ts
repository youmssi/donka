import { expect, test } from './fixtures';
import { settings } from './settings';

test.use({ storageState: { cookies: [], origins: [] } });

test('Studio API reference uses the installed OpenAPI and same-origin assets', async ({ page }) => {
  const { studio } = settings();
  const externalRequests: string[] = [];
  page.on('request', (request) => {
    if (new URL(request.url()).origin !== new URL(studio).origin) externalRequests.push(request.url());
  });

  const response = await page.request.get(new URL('/api/v1/openapi.json', studio).href);
  await expect(response).toBeOK();
  const openapi = await response.json();

  await page.goto(new URL('/api/docs', studio).href);
  await expect(page.getByText('Donka Studio API', { exact: true })).toBeVisible();
  await expect(page.getByText('/api/v1/health', { exact: true })).toBeVisible();
  await expect(page.locator('script[src="/scalar-api-reference.js"]')).toHaveCount(1);
  expect(openapi.info.version).toBeTruthy();
  expect(externalRequests).toEqual([]);
});
