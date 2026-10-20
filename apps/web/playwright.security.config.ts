import { defineConfig } from '@playwright/test';
import config from './playwright.config';

// Run after restarting the isolated service with the small limits documented in live-tests.md.
export default defineConfig({
  ...config,
  testDir: './security-probes',
  globalSetup: './security-probes/global-setup.ts',
});
