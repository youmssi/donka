import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { expect } from './fixtures';
import { required, settings } from './settings';

/** Execute the built CLI with only its credential, never the test administrator's environment. */
export async function runCli(
  cwd: string,
  project: string,
  token: string,
  args: string[],
  target = 'main',
): Promise<{ code: number | null; stdout: string; stderr: string }> {
  const environment = Object.fromEntries(
    ['PATH', 'HOME', 'USERPROFILE', 'SystemRoot', 'TMP', 'TEMP'].flatMap((name) => {
      const value = process.env[name];
      return value ? [[name, value]] : [];
    }),
  );
  const result = await new Promise<{ code: number | null; stdout: string; stderr: string }>((resolve, reject) => {
    const child = spawn(process.execPath, [join(required('DONKA_E2E_CLI_DIR'), 'dist/main.js'), ...args], {
      cwd,
      env: {
        ...environment,
        NODE_ENV: 'test',
        NO_COLOR: '1',
        DONKA_URL: settings().studio,
        DONKA_PROJECT: project,
        DONKA_TOKEN: token,
        DONKA_TARGET: target,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 60_000,
      windowsHide: true,
    });
    let stdout = '',
      stderr = '';
    child.stdout.on('data', (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    child.once('error', reject);
    child.once('close', (code) => resolve({ code, stdout, stderr }));
  });
  // A failed privacy assertion must not echo the secret as an expected value.
  expect(
    Boolean(token && (result.stdout.includes(token) || result.stderr.includes(token))),
    'CLI output contains no credential',
  ).toBe(false);
  return result;
}
