import { expect, request } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { settings } from '../e2e/settings';
import { assertLimited, budget } from './limits';

/** Proves the fresh process enforces exactly the configured burst before browser cases spend it. */
export default async function setup() {
  const api = await request.newContext({ baseURL: settings().studio, extraHTTPHeaders: { 'x-donka-csrf': '1' } });
  try {
    await expect(await api.get('/api/v1/ready')).toBeOK();
    const email = `limit-setup-${randomUUID()}@donka.local`;
    for (const [path, count, data, status] of [
      [
        '/api/v1/auth/sign-in',
        budget('DONKA_E2E_SIGN_IN_RATE_LIMIT'),
        { email, password: 'disposable rate probe passphrase' },
        401,
      ],
      ['/api/v1/auth/password-reset', budget('DONKA_E2E_PASSWORD_RESET_RATE_LIMIT'), { email }, 202],
    ] as const) {
      for (let attempt = 0; attempt < count; attempt++) expect((await api.post(path, { data })).status()).toBe(status);
      await assertLimited(await api.post(path, { data }));
    }
  } finally {
    await api.dispose();
  }
}
