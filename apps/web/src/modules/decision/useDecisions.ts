'use client';

import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  createDecision,
  deleteDecision,
  getDecision,
  getVersion,
  listDecisions,
  listVersions,
  VERSIONS_PAGE_SIZE,
} from './decision.service';
import type { CreateDecisionValues } from './schema';

const keys = {
  list: (projectId: string) => ['project', projectId, 'decisions'] as const,
  one: (projectId: string, id: string) => ['project', projectId, 'decisions', id] as const,
  versions: (projectId: string, id: string) => ['project', projectId, 'decisions', id, 'versions'] as const,
  version: (projectId: string, id: string, number: number) =>
    ['project', projectId, 'decisions', id, 'versions', number] as const,
};

/** The decision's history, newest first, a page at a time ("Load more"). */
export function useVersions(projectId: string, id: string, enabled: boolean) {
  return useInfiniteQuery({
    queryKey: keys.versions(projectId, id),
    queryFn: ({ pageParam }) => listVersions(projectId, id, pageParam),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      if (!last.ok) return undefined;
      const loaded = pages.length * VERSIONS_PAGE_SIZE;
      return loaded < last.data.total ? loaded : undefined;
    },
    enabled,
  });
}

/** One version with its content; versions never change, so it is fetched once. */
export function useVersion(projectId: string, id: string, number: number | null) {
  return useQuery({
    queryKey: keys.version(projectId, id, number ?? 0),
    queryFn: () => getVersion(projectId, id, number ?? 0),
    enabled: number !== null,
    staleTime: Infinity,
  });
}

/** After a version is saved or restored, the history and the list show it. */
export function useVersionsChanged(projectId: string, id: string) {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: keys.versions(projectId, id) });
    void queryClient.invalidateQueries({ queryKey: keys.list(projectId), exact: true });
  };
}

export function useDecisionList(projectId: string) {
  return useQuery({ queryKey: keys.list(projectId), queryFn: () => listDecisions(projectId) });
}

export function useDecision(projectId: string, id: string | undefined) {
  return useQuery({
    queryKey: keys.one(projectId, id ?? ''),
    queryFn: () => getDecision(projectId, id ?? ''),
    enabled: Boolean(id),
    // The editor owns the draft once loaded: a refetch must not replace what is being edited.
    staleTime: Infinity,
    gcTime: 0,
  });
}

export function useCreateDecision(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: CreateDecisionValues) => createDecision(projectId, values),
    onSuccess: (result) => {
      if (result.ok) void queryClient.invalidateQueries({ queryKey: keys.list(projectId) });
    },
  });
}

export function useDeleteDecision(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteDecision(projectId, id),
    onSuccess: (result) => {
      if (result.ok) void queryClient.invalidateQueries({ queryKey: keys.list(projectId) });
    },
  });
}

/** After a save, the list shows the new time and author. */
export function useDecisionSaved(projectId: string) {
  const queryClient = useQueryClient();
  return () => void queryClient.invalidateQueries({ queryKey: keys.list(projectId), exact: true });
}
