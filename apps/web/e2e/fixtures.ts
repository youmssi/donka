import { test as base, expect, type APIRequestContext, type Page, type BrowserContext } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { z } from 'zod';

import en from '../src/messages/en.json';
import fr from '../src/messages/fr.json';
import { origin, required, settings } from './settings';

export type Messages = typeof en;

export interface UiOptions {
  appLocale: 'en' | 'fr';
  theme: 'light' | 'dark';
}
interface Fixtures extends UiOptions {
  messages: typeof en;
  admin: APIRequestContext;
  services: APIRequestContext;
  appearance: void;
  pageErrors: void;
  reviewer: BrowserContext;
  explaining: BrowserContext;
}

async function appearance(context: BrowserContext, theme: UiOptions['theme'], studio = settings().studio) {
  await context.addInitScript(
    ({ origin, value }) => {
      // Editor sandboxes have opaque origins and must not access the app's storage.
      if (window === window.top && location.origin === origin) localStorage.setItem('theme', value);
    },
    { origin: studio, value: theme },
  );
}

function pageErrors(context: BrowserContext) {
  const errors: string[] = [];
  const listen = (page: Page) => page.on('pageerror', (error) => errors.push(error.stack ?? error.message));
  context.on('page', listen);
  context.pages().forEach(listen);
  return errors;
}

export const test = base.extend<Fixtures>({
  appLocale: ['en', { option: true }],
  theme: ['light', { option: true }],
  storageState: async ({}, run) => run(required('DONKA_E2E_AUTH_FILE')),
  messages: async ({ appLocale }, run) => run(appLocale === 'fr' ? fr : en),
  admin: async ({ playwright }, run) => {
    const api = await playwright.request.newContext({
      baseURL: settings().studio,
      storageState: required('DONKA_E2E_AUTH_FILE'),
      extraHTTPHeaders: { 'x-donka-csrf': '1' },
    });
    try {
      await run(api);
    } finally {
      await api.dispose();
    }
  },
  services: async ({ playwright }, run) => {
    const api = await playwright.request.newContext({ storageState: { cookies: [], origins: [] } });
    try {
      await run(api);
    } finally {
      await api.dispose();
    }
  },
  reviewer: async ({ browser, appLocale, theme }, run, testInfo) => {
    const context = await browser.newContext({
      baseURL: settings().studio,
      storageState: { cookies: [], origins: [] },
      viewport: testInfo.project.use.viewport,
      locale: appLocale,
      colorScheme: theme,
    });
    await appearance(context, theme);
    const errors = pageErrors(context);
    try {
      await run(context);
    } finally {
      await context.close();
      expect(errors, 'No reviewer JavaScript page errors').toEqual([]);
    }
  },
  explaining: async ({ browser, appLocale, theme }, run, testInfo) => {
    const studio = origin('DONKA_E2E_EXPLAIN_STUDIO_URL');
    const context = await browser.newContext({
      baseURL: studio,
      storageState: required('DONKA_E2E_AUTH_FILE'),
      viewport: testInfo.project.use.viewport,
      locale: appLocale,
      colorScheme: theme,
    });
    await appearance(context, theme, studio);
    const errors = pageErrors(context);
    try {
      await run(context);
    } finally {
      await context.close();
      expect(errors, 'No explanation-instance JavaScript page errors').toEqual([]);
    }
  },
  appearance: [
    async ({ context, theme }, run) => {
      await appearance(context, theme);
      await run();
    },
    { auto: true },
  ],
  pageErrors: [
    async ({ context }, run) => {
      const errors = pageErrors(context);
      await run();
      expect(errors, 'No unexpected JavaScript page errors').toEqual([]);
    },
    { auto: true },
  ],
});
export { expect };

export function unique(prefix: string): string {
  return `${prefix}-${randomUUID().slice(0, 8)}`;
}

/** Invited/activated account fixture; the journey signs in through the real UI. */
export async function newUser(
  admin: APIRequestContext,
  services: APIRequestContext,
  locale: UiOptions['appLocale'],
  isAdmin = false,
) {
  const email = `${unique('journey')}@donka.local`;
  const password = 'disposable feature journey passphrase';
  await expect(await admin.post('/api/v1/users/invitations', { data: { email, locale, isAdmin } })).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', {
      data: { token: await passwordLink(services, email), password },
    }),
  ).toBeOK();
  return { email, password };
}

const mailList = z.object({
  messages: z.array(
    z.object({
      ID: z.string(),
      To: z.array(z.object({ Address: z.string() })),
    }),
  ),
});
const mailBody = z.object({ Text: z.string().optional(), HTML: z.string().optional() });

export async function passwordLink(api: APIRequestContext, email: string, previous?: string): Promise<string> {
  let token: string | undefined;
  await expect
    .poll(
      async () => {
        const list = await api.get(`${settings().mailpit}/api/v1/messages`);
        await expect(list).toBeOK();
        for (const message of mailList.parse(await list.json()).messages) {
          if (!message.To.some((to) => to.Address === email)) continue;
          const response = await api.get(`${settings().mailpit}/api/v1/message/${message.ID}`);
          await expect(response).toBeOK();
          const body = mailBody.parse(await response.json());
          const found = (body.Text ?? body.HTML ?? '').match(/setup-password\/?\?token=([A-Za-z0-9_-]+)/)?.[1];
          if (found && found !== previous) {
            token = found;
            return true;
          }
        }
        return false;
      },
      { timeout: 60_000, message: 'The test recipient receives the invitation/reset in Mailpit' },
    )
    .toBe(true);
  if (!token) throw new Error('The matching password link was not found');
  return token;
}
