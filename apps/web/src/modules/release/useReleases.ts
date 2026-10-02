'use client';

import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  getApproval,
  getRelease,
  listApprovals,
  listEnvironments,
  listRollbackTargets,
  listReleases,
  listTokens,
  previewRelease,
} from './release.service';
import { inFlight, type EnvironmentName } from './schema';

/** While a deployment is on its way, the environments are asked again this often. */
const IN_FLIGHT_POLL_MS = 2000;

const keys = {
  all: (projectId: string) => ['project', projectId, 'releases'] as const,
  list: (projectId: string, offset: number) => ['project', projectId, 'releases', 'list', offset] as const,
  one: (projectId: string, id: string) => ['project', projectId, 'releases', 'one', id] as const,
  preview: (projectId: string) => ['project', projectId, 'releases', 'preview'] as const,
  environments: (projectId: string) => ['project', projectId, 'releases', 'environments'] as const,
  tokens: (projectId: string, env: EnvironmentName) => ['project', projectId, 'releases', 'tokens', env] as const,
  approvals: (projectId: string, offset: number) => ['project', projectId, 'releases', 'approvals', offset] as const,
  approval: (projectId: string, id: string) => ['project', projectId, 'releases', 'approval', id] as const,
  rollbackTargets: (projectId: string) => ['project', projectId, 'releases', 'rollback-targets'] as const,
};

export function useReleaseList(projectId: string, offset: number) {
  return useQuery({
    queryKey: keys.list(projectId, offset),
    queryFn: () => listReleases(projectId, offset),
    placeholderData: keepPreviousData,
  });
}

/** A release never changes once made. */
export function useRelease(projectId: string, id: string | null) {
  return useQuery({
    queryKey: keys.one(projectId, id ?? ''),
    queryFn: () => getRelease(projectId, id ?? ''),
    enabled: id !== null,
    staleTime: Infinity,
  });
}

export function useReleasePreview(projectId: string, enabled: boolean) {
  return useQuery({ queryKey: keys.preview(projectId), queryFn: () => previewRelease(projectId), enabled });
}

/** Both environments; asked again while a deployment is on its way. */
export function useEnvironments(projectId: string) {
  return useQuery({
    queryKey: keys.environments(projectId),
    queryFn: () => listEnvironments(projectId),
    refetchInterval: (query) => {
      const result = query.state.data;
      return result?.ok && result.data.some((env) => inFlight(env.latest)) ? IN_FLIGHT_POLL_MS : false;
    },
  });
}

export function useTokens(projectId: string, env: EnvironmentName) {
  return useQuery({ queryKey: keys.tokens(projectId, env), queryFn: () => listTokens(projectId, env) });
}

export function useApprovals(projectId: string, offset: number) {
  return useQuery({
    queryKey: keys.approvals(projectId, offset),
    queryFn: () => listApprovals(projectId, offset),
    placeholderData: keepPreviousData,
  });
}

export function useApproval(projectId: string, id: string | null) {
  return useQuery({
    queryKey: keys.approval(projectId, id ?? ''),
    queryFn: () => getApproval(projectId, id ?? ''),
    enabled: id !== null,
  });
}

export function useRollbackTargets(projectId: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.rollbackTargets(projectId),
    queryFn: () => listRollbackTargets(projectId),
    enabled,
  });
}

/** After a release, a deployment, an approval or a token change, every view of them updates. */
export function useReleasesChanged(projectId: string) {
  const queryClient = useQueryClient();
  return () => void queryClient.invalidateQueries({ queryKey: keys.all(projectId) });
}
