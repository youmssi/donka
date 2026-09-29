'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { createDecision, deleteDecision, getDecision, listDecisions } from './decision.service';
import type { CreateDecisionValues } from './schema';

const keys = {
  list: (projectId: string) => ['project', projectId, 'decisions'] as const,
  one: (projectId: string, id: string) => ['project', projectId, 'decisions', id] as const,
};

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
