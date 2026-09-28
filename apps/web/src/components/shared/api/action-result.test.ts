import { attempt, failure } from './action-result';

function jsonResponse(status: number, body: unknown, headers: Record<string, string> = {}) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } });
}

describe('failure', () => {
  it('keeps a known code, the request id and the refused fields', async () => {
    const result = await failure(
      jsonResponse(400, { code: 'INVALID_REQUEST', message: 'x', requestId: 'r-1', fields: { email: 'bad' } }),
    );
    expect(result.error).toEqual({ code: 'INVALID_REQUEST', requestId: 'r-1', fieldErrors: { email: 'bad' } });
  });

  it('turns a code the web app does not know into UNEXPECTED', async () => {
    const result = await failure(jsonResponse(409, { code: 'SOMETHING_NEW', message: 'x', requestId: 'r-2' }));
    expect(result.error.code).toBe('UNEXPECTED');
    expect(result.error.requestId).toBe('r-2');
  });

  it('copes with a body that is not the API error shape', async () => {
    const result = await failure(
      new Response('<html>Bad gateway</html>', { status: 502, headers: { 'x-request-id': 'r-3' } }),
    );
    expect(result.error).toEqual({ code: 'UNEXPECTED', requestId: 'r-3', fieldErrors: undefined });
  });
});

describe('attempt', () => {
  it('reports an unreachable server as NETWORK instead of throwing', async () => {
    const result = await attempt(() => Promise.reject(new TypeError('Failed to fetch')));
    expect(result).toEqual({ ok: false, error: { code: 'NETWORK' } });
  });
});
