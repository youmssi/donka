import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  Approval,
  ApprovalList,
  ApprovalReview,
  Deployment,
  EnvironmentName,
  EnvironmentState,
  IssuedToken,
  Release,
  ReleaseList,
  ReleasePreview,
  ReleaseSummary,
  ReleaseValues,
  RuntimeToken,
} from './schema';

const project = (projectId: string) => `projects/${encodeURIComponent(projectId)}`;
const environment = (projectId: string, env: EnvironmentName) => `${project(projectId)}/environments/${env}`;

export const RELEASES_PAGE_SIZE = 50;
export const APPROVALS_PAGE_SIZE = 50;

export function listReleases(projectId: string, offset: number): Promise<ActionResult<ReleaseList>> {
  return attempt(async () => {
    const response = await http.get(`${project(projectId)}/releases`, {
      searchParams: { limit: RELEASES_PAGE_SIZE, offset },
    });
    return response.ok ? success(await response.json<ReleaseList>()) : failure(response);
  });
}

export function getRelease(projectId: string, id: string): Promise<ActionResult<Release>> {
  return attempt(async () => {
    const response = await http.get(`${project(projectId)}/releases/${encodeURIComponent(id)}`);
    return response.ok ? success(await response.json<Release>()) : failure(response);
  });
}

export function previewRelease(projectId: string): Promise<ActionResult<ReleasePreview>> {
  return attempt(async () => {
    const response = await http.get(`${project(projectId)}/releases/preview`);
    return response.ok ? success(await response.json<ReleasePreview>()) : failure(response);
  });
}

export function createRelease(projectId: string, values: ReleaseValues): Promise<ActionResult<Release>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/releases`, { json: values });
    return response.ok ? success(await response.json<Release>()) : failure(response);
  });
}

export function listEnvironments(projectId: string): Promise<ActionResult<EnvironmentState[]>> {
  return attempt(async () => {
    const response = await http.get(`${project(projectId)}/environments`);
    return response.ok ? success((await response.json<{ items: EnvironmentState[] }>()).items) : failure(response);
  });
}

/** Queues the release for the environment; the artifact is written shortly after. */
export function deployRelease(
  projectId: string,
  env: EnvironmentName,
  releaseId: string,
): Promise<ActionResult<Deployment>> {
  return attempt(async () => {
    const response = await http.post(`${environment(projectId, env)}/deployments`, { json: { releaseId } });
    return response.ok ? success(await response.json<Deployment>()) : failure(response);
  });
}

export function retryDeployment(
  projectId: string,
  env: EnvironmentName,
  id: string,
): Promise<ActionResult<Deployment>> {
  return attempt(async () => {
    const response = await http.post(`${environment(projectId, env)}/deployments/${encodeURIComponent(id)}/retry`);
    return response.ok ? success(await response.json<Deployment>()) : failure(response);
  });
}

export function listTokens(projectId: string, env: EnvironmentName): Promise<ActionResult<RuntimeToken[]>> {
  return attempt(async () => {
    const response = await http.get(`${environment(projectId, env)}/tokens`);
    return response.ok ? success((await response.json<{ items: RuntimeToken[] }>()).items) : failure(response);
  });
}

export function issueToken(projectId: string, env: EnvironmentName, name: string): Promise<ActionResult<IssuedToken>> {
  return attempt(async () => {
    const response = await http.post(`${environment(projectId, env)}/tokens`, { json: { name } });
    return response.ok ? success(await response.json<IssuedToken>()) : failure(response);
  });
}

export function revokeToken(projectId: string, env: EnvironmentName, id: string): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.delete(`${environment(projectId, env)}/tokens/${encodeURIComponent(id)}`);
    return response.ok ? success(null) : failure(response);
  });
}

const approvals = (projectId: string) => `${project(projectId)}/approvals`;
const approval = (projectId: string, id: string) => `${approvals(projectId)}/${encodeURIComponent(id)}`;

export function listApprovals(projectId: string, offset: number): Promise<ActionResult<ApprovalList>> {
  return attempt(async () => {
    const response = await http.get(approvals(projectId), { searchParams: { limit: APPROVALS_PAGE_SIZE, offset } });
    return response.ok ? success(await response.json<ApprovalList>()) : failure(response);
  });
}

export function getApproval(projectId: string, id: string): Promise<ActionResult<ApprovalReview>> {
  return attempt(async () => {
    const response = await http.get(approval(projectId, id));
    return response.ok ? success(await response.json<ApprovalReview>()) : failure(response);
  });
}

/** Asks for the release live on staging to go to production; owners are emailed. */
export function requestApproval(projectId: string, releaseId: string): Promise<ActionResult<Approval>> {
  return attempt(async () => {
    const response = await http.post(approvals(projectId), { json: { releaseId } });
    return response.ok ? success(await response.json<Approval>()) : failure(response);
  });
}

export function approve(projectId: string, id: string): Promise<ActionResult<Approval>> {
  return attempt(async () => {
    const response = await http.post(`${approval(projectId, id)}/approve`);
    return response.ok ? success(await response.json<Approval>()) : failure(response);
  });
}

export function reject(projectId: string, id: string, reason: string): Promise<ActionResult<Approval>> {
  return attempt(async () => {
    const response = await http.post(`${approval(projectId, id)}/reject`, { json: { reason } });
    return response.ok ? success(await response.json<Approval>()) : failure(response);
  });
}

export function withdraw(projectId: string, id: string): Promise<ActionResult<Approval>> {
  return attempt(async () => {
    const response = await http.post(`${approval(projectId, id)}/withdraw`);
    return response.ok ? success(await response.json<Approval>()) : failure(response);
  });
}

/** The releases production can go back to: approved once, not live now. */
export function listRollbackTargets(projectId: string): Promise<ActionResult<ReleaseSummary[]>> {
  return attempt(async () => {
    const response = await http.get(`${project(projectId)}/rollback-targets`);
    return response.ok ? success((await response.json<{ items: ReleaseSummary[] }>()).items) : failure(response);
  });
}

/** Puts an approved release back in production, with a reason and without a new approval. */
export function rollback(projectId: string, releaseId: string, reason: string): Promise<ActionResult<Deployment>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/rollbacks`, { json: { releaseId, reason } });
    return response.ok ? success(await response.json<Deployment>()) : failure(response);
  });
}
