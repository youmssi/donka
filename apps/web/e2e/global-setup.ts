import { expect, request } from '@playwright/test';
import { mkdtemp, rmdir, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { settings } from './settings';

export default async function setup() {
  const environment = settings();
  const api = await request.newContext({ baseURL: environment.studio, extraHTTPHeaders: { 'x-donka-csrf': '1' } });
  const directory = await mkdtemp(join(tmpdir(), 'donka-e2e-'));
  const authFile = join(directory, 'admin.json');
  let signedIn = false;
  let saved = false;
  async function cleanup() {
    try {
      if (signedIn) await expect(await api.post('/api/v1/auth/sign-out')).toBeOK();
    } finally {
      await api.dispose();
      if (saved) await unlink(authFile);
      await rmdir(directory);
    }
  }
  try {
    for (const url of [`${environment.studio}/api/v1/ready`, `${environment.mailpit}/api/v1/messages`])
      await expect(await api.get(url)).toBeOK();
    await expect(
      await api.post('/api/v1/auth/sign-in', {
        data: { email: environment.email, password: environment.password },
      }),
    ).toBeOK();
    signedIn = true;
    await writeFile(authFile, JSON.stringify(await api.storageState()), { mode: 0o600 });
    saved = true;
    // Only the private temporary path reaches workers; it is outside report/artifact directories.
    process.env.DONKA_E2E_AUTH_FILE = authFile;
    return cleanup;
  } catch (error) {
    await cleanup();
    throw error;
  }
}
