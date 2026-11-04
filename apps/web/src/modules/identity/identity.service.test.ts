import { stubBrowserFetch } from '@/test/browser-fetch';

import * as identity from './identity.service';

const fetchMock = vi.fn<typeof fetch>();
let requests: Request[];
const user = { id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' };
const values = { email: user.email, password: 'synthetic password' };

function reply(body: unknown, status = 200) {
  fetchMock.mockResolvedValueOnce(
    new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
    }),
  );
}

function sentRequest() {
  const request = requests.at(-1);
  if (!request) throw new Error('Expected the shared HTTP client to send a Request');
  return request;
}

beforeEach(() => {
  fetchMock.mockReset();
  requests = stubBrowserFetch(fetchMock);
});
afterEach(() => vi.unstubAllGlobals());

it('distinguishes an expired session from an unavailable identity service', async () => {
  reply(user);
  expect(await identity.getSession()).toEqual({ ok: true, data: user });
  reply({ code: 'UNAUTHENTICATED' }, 401);
  expect(await identity.getSession()).toEqual({ ok: true, data: null });
  reply({ code: 'DATABASE_UNAVAILABLE', requestId: 'session-1' }, 503);
  expect(await identity.getSession()).toMatchObject({
    ok: false,
    error: { code: 'DATABASE_UNAVAILABLE', requestId: 'session-1' },
  });
});

it('opens a session only on a complete sign-in, retaining each required second step', async () => {
  reply(user);
  expect(await identity.signIn(values)).toEqual({ ok: true, data: { step: 'signedIn', user } });
  const request = sentRequest();
  expect(new URL(request.url).pathname).toBe('/api/v1/auth/sign-in');
  expect(request.method).toBe('POST');
  expect(request.credentials).toBe('same-origin');
  expect(request.headers.get('x-donka-csrf')).toBe('1');
  expect(await request.clone().json()).toEqual(values);
  for (const next of ['code', 'enrol']) {
    reply({ next }, 202);
    expect(await identity.signIn(values)).toEqual({ ok: true, data: { step: next } });
  }
});

it('preserves refused credentials and transport failures without opening a session', async () => {
  reply({ code: 'INVALID_CREDENTIALS', requestId: 'sign-in-1' }, 401);
  expect(await identity.signIn(values)).toMatchObject({
    ok: false,
    error: { code: 'INVALID_CREDENTIALS', requestId: 'sign-in-1' },
  });
  fetchMock.mockRejectedValueOnce(new TypeError('Connection refused'));
  expect(await identity.signIn(values)).toEqual({ ok: false, error: { code: 'NETWORK' } });
});

it('finishes the second step only after the server accepts the code', async () => {
  reply({ code: 'INVALID_CODE', requestId: 'code-1' }, 401);
  expect(await identity.completeSignIn({ code: '000000' })).toMatchObject({
    ok: false,
    error: { code: 'INVALID_CODE' },
  });
  reply(user);
  expect(await identity.completeSignIn({ code: '123456' })).toEqual({ ok: true, data: user });
});

it('keeps sign-in enrolment recovery codes attached to the newly authenticated user', async () => {
  const enrolment = { secret: 'synthetic', uri: 'otpauth://synthetic' };
  reply(enrolment);
  expect(await identity.startSignInEnrolment()).toEqual({ ok: true, data: enrolment });
  const accepted = { user, recoveryCodes: ['synthetic-code'] };
  reply(accepted);
  expect(await identity.confirmSignInEnrolment({ code: '123456' })).toEqual({ ok: true, data: accepted });
  reply({ code: 'SIGN_IN_EXPIRED' }, 401);
  expect(await identity.confirmSignInEnrolment({ code: '123456' })).toMatchObject({
    ok: false,
    error: { code: 'SIGN_IN_EXPIRED' },
  });
});

it('unwraps recovery codes and never presents a refused regeneration as successful', async () => {
  reply({ recoveryCodes: ['one', 'two'] });
  expect(await identity.confirmEnrolment({ code: '123456' })).toEqual({ ok: true, data: ['one', 'two'] });
  reply({ code: 'INVALID_CODE' }, 401);
  expect(await identity.regenerateRecoveryCodes({ code: '000000' })).toMatchObject({
    ok: false,
    error: { code: 'INVALID_CODE' },
  });
  reply({ recoveryCodes: ['replacement'] });
  expect(await identity.regenerateRecoveryCodes({ code: '123456' })).toEqual({ ok: true, data: ['replacement'] });
});

it('treats an already expired sign-out as complete, while retaining other server failures', async () => {
  reply({}, 401);
  expect(await identity.signOut()).toEqual({ ok: true, data: null });
  reply({ code: 'DATABASE_UNAVAILABLE' }, 503);
  expect(await identity.signOut()).toMatchObject({ ok: false, error: { code: 'DATABASE_UNAVAILABLE' } });
});

it('reads account enrolment and requires an accepted code before disabling the second step', async () => {
  const status = { enabled: true, recoveryCodesRemaining: 8 };
  reply(status);
  expect(await identity.getSecondStep()).toEqual({ ok: true, data: status });
  const enrolment = { secret: 'synthetic', uri: 'otpauth://synthetic' };
  reply(enrolment);
  expect(await identity.startEnrolment()).toEqual({ ok: true, data: enrolment });
  reply({ code: 'INVALID_CODE' }, 401);
  expect(await identity.disableSecondStep({ code: '000000' })).toMatchObject({
    ok: false,
    error: { code: 'INVALID_CODE' },
  });
  reply({});
  expect(await identity.disableSecondStep({ code: '123456' })).toEqual({ ok: true, data: null });
});

it('keeps password-reset confirmation anonymous and reports expired setup links', async () => {
  reply({}, 202);
  expect(await identity.requestPasswordReset({ email: user.email })).toEqual({ ok: true, data: null });
  reply({ code: 'INVALID_SETUP_LINK' }, 400);
  expect(await identity.setPassword('expired', values.password)).toMatchObject({
    ok: false,
    error: { code: 'INVALID_SETUP_LINK' },
  });
  reply({});
  expect(await identity.setPassword('synthetic-token', values.password)).toEqual({ ok: true, data: null });
});
