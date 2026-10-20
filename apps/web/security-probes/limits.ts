import { expect, type APIResponse } from '@playwright/test';
import { required } from '../e2e/settings';

export function budget(name: string): number {
  const value = Number(required(name));
  if (!Number.isInteger(value) || value < 1 || value > 5) {
    throw new Error(`${name} must match an isolated service limit between 1 and 5 requests per minute.`);
  }
  return value;
}

export async function assertLimited(reply: Pick<APIResponse, 'status' | 'headers' | 'json'>): Promise<number> {
  expect(reply.status()).toBe(429);
  expect(reply.headers()['set-cookie']).toBeUndefined();
  const seconds = Number(reply.headers()['retry-after']);
  expect(seconds).toBeGreaterThan(0);
  expect(seconds).toBeLessThanOrEqual(60);
  expect(await reply.json()).toMatchObject({ code: 'RATE_LIMITED', details: { retryAfterSeconds: seconds } });
  return seconds;
}
