import { vi } from 'vitest';

/** Node's Request needs the URL resolution that browser fetch provides for same-origin paths. */
export function stubBrowserFetch(fetch: typeof globalThis.fetch) {
  const NativeRequest = globalThis.Request;
  const requests: Request[] = [];
  class BrowserRequest extends NativeRequest {
    constructor(input: RequestInfo | URL, init?: RequestInit) {
      super(typeof input === 'string' ? new URL(input, window.location.href) : input, init);
    }
  }
  vi.stubGlobal('Request', BrowserRequest);
  vi.stubGlobal('fetch', (input: RequestInfo | URL, init?: RequestInit) => {
    requests.push(input instanceof NativeRequest ? input.clone() : new BrowserRequest(input, init));
    return fetch(input, init);
  });
  return requests;
}
