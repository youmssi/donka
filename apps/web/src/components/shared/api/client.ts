import ky from 'ky';

import type { ErrorBody } from './types';

/** Same origin as the pages: the session cookie travels on its own (frontend.md §3). */
export const API_BASE = '/api/v1';
const CSRF_HEADER = 'x-donka-csrf';
const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS']);

/**
 * The one HTTP client. It never throws on an HTTP status: services turn every
 * response into an ActionResult. It throws only when the server cannot be reached.
 */
export const http = ky.create({
  prefix: API_BASE,
  credentials: 'same-origin',
  timeout: 15_000,
  retry: 0,
  throwHttpErrors: false,
  hooks: {
    beforeRequest: [
      ({ request }) => {
        if (!SAFE_METHODS.has(request.method)) request.headers.set(CSRF_HEADER, '1');
      },
    ],
  },
});

/** The API's error body, or null when the response is not one (e.g. a proxy error page). */
export async function readErrorBody(response: Response): Promise<ErrorBody | null> {
  try {
    const body: unknown = await response.json();
    if (body && typeof body === 'object' && 'code' in body && typeof body.code === 'string') {
      return body as ErrorBody;
    }
  } catch {
    // Not JSON: fall through.
  }
  return null;
}
