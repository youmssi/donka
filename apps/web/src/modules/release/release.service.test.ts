import { stubBrowserFetch } from '@/test/browser-fetch';

import {
  approve,
  deployRelease,
  listEnvironments,
  listRollbackTargets,
  reject,
  revokeToken,
  rollback,
} from './release.service';

const fetchMock = vi.fn<typeof fetch>();
let requests: Request[];
beforeEach(() => {
  fetchMock.mockReset();
  requests = stubBrowserFetch(fetchMock);
});
afterEach(() => vi.unstubAllGlobals());

function reply(body: unknown, status = 200) {
  fetchMock.mockResolvedValueOnce(
    new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } }),
  );
}

function sentRequest() {
  const request = requests.at(-1);
  if (!request) throw new Error('Expected a shared-client Request');
  return request;
}

it('retains the self-approval boundary and server request reference', async () => {
  reply({ code: 'SELF_APPROVAL', requestId: 'approval-1' }, 403);
  expect(await approve('project/one', 'approval/two')).toMatchObject({
    ok: false,
    error: { code: 'SELF_APPROVAL', requestId: 'approval-1' },
  });
  expect(new URL(sentRequest().url).pathname).toBe('/api/v1/projects/project%2Fone/approvals/approval%2Ftwo/approve');
});

it('sends the rejection reason and keeps an already decided approval as a failure', async () => {
  reply({ code: 'APPROVAL_DECIDED' }, 409);
  expect(await reject('project', 'approval', 'Scenario disagrees')).toMatchObject({
    ok: false,
    error: { code: 'APPROVAL_DECIDED' },
  });
  expect(await sentRequest().clone().json()).toEqual({ reason: 'Scenario disagrees' });
});

it('unwraps approved rollback choices and sends the selected release and reason', async () => {
  const target = { id: 'release-1', version: '1.0.0' };
  reply({ items: [target] });
  expect(await listRollbackTargets('project')).toEqual({ ok: true, data: [target] });
  const deployment = { id: 'deployment-1', status: 'pending' };
  reply(deployment, 202);
  expect(await rollback('project', target.id, 'Restore approved behavior')).toEqual({ ok: true, data: deployment });
  expect(await sentRequest().clone().json()).toEqual({ releaseId: target.id, reason: 'Restore approved behavior' });
  reply({ code: 'NEVER_APPROVED' }, 409);
  expect(await rollback('project', 'other', 'Refused')).toMatchObject({ ok: false, error: { code: 'NEVER_APPROVED' } });
});

it('unwraps environment cards and handles token revocation without requiring a response body', async () => {
  reply({ items: [{ name: 'production', live: null }] });
  expect(await listEnvironments('project')).toEqual({ ok: true, data: [{ name: 'production', live: null }] });
  fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));
  expect(await revokeToken('project', 'production', 'token/id')).toEqual({ ok: true, data: null });
  const request = sentRequest();
  expect(request.method).toBe('DELETE');
  expect(new URL(request.url).pathname).toBe('/api/v1/projects/project/environments/production/tokens/token%2Fid');
});

it('distinguishes a queued staging deployment from refused production publication', async () => {
  const deployment = { id: 'deployment-1', status: 'pending' };
  reply(deployment, 202);
  expect(await deployRelease('project', 'staging', 'release-1')).toEqual({ ok: true, data: deployment });
  expect(await sentRequest().clone().json()).toEqual({ releaseId: 'release-1' });
  reply({ code: 'APPROVAL_REQUIRED' }, 409);
  expect(await deployRelease('project', 'production', 'release-1')).toMatchObject({
    ok: false,
    error: { code: 'APPROVAL_REQUIRED' },
  });
});
