import { defineConfig } from '@playwright/test';
import type { UiOptions } from './e2e/fixtures';
import { settings } from './e2e/settings';

const environment = settings();
const browsers = ['chromium', 'firefox', 'webkit'] as const;

export default defineConfig<UiOptions>({
  testDir: './e2e',
  globalSetup: './e2e/global-setup.ts',
  fullyParallel: false,
  workers: 1,
  forbidOnly: true,
  retries: 0,
  timeout: 150_000,
  expect: { timeout: 15_000 },
  outputDir: 'test-results',
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: environment.studio,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: browsers.flatMap((browserName) =>
    (['en', 'fr'] as const).flatMap((appLocale) =>
      (['light', 'dark'] as const).flatMap((theme) =>
        [1440, 390].map((width) => ({
          name: `${browserName}-${appLocale}-${theme}-${width}`,
          use: {
            browserName,
            channel: browserName === 'chromium' ? process.env.DONKA_TEST_BROWSER_CHANNEL : undefined,
            appLocale,
            theme,
            locale: appLocale,
            colorScheme: theme,
            viewport: { width, height: width === 390 ? 844 : 1000 },
          },
        })),
      ),
    ),
  ),
});
