import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  Deployment,
  EnvironmentName,
  EnvironmentState,
  IssuedToken,
  Release,
  ReleaseList,
  ReleasePreview,
  ReleaseValues,
  RuntimeToken,
} from './schema';

const project = (projectId: string) => `projects/${encodeURIComponent(projectId)}`;
const environment = (projectId: string, env: EnvironmentName) => `${project(projectId)}/environments/${env}`;

export const RELEASES_PAGE_SIZE = 50;

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
