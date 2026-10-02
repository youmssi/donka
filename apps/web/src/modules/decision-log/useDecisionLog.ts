'use client';

import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { getRecord, getSettings, listTokens, replayRecord, saveSettings, searchRecords } from './decision-log.service';
import type { LogQuery, LogSettings } from './schema';

const keys = {
  search: (projectId: string, query: LogQuery, offset: number) =>
    ['project', projectId, 'decision-log', 'search', query, offset] as const,
  record: (projectId: string, id: string) => ['project', projectId, 'decision-log', 'record', id] as const,
  settings: (projectId: string) => ['project', projectId, 'decision-log', 'settings'] as const,
  tokens: ['decision-log-tokens'] as const,
};

export function useRecords(projectId: string, query: LogQuery, offset: number, enabled: boolean) {
  return useQuery({
    queryKey: keys.search(projectId, query, offset),
    queryFn: () => searchRecords(projectId, query, offset),
    placeholderData: keepPreviousData,
    enabled,
  });
}

/** A record never changes; opening it is audited, so it is asked for once per visit. */
export function useRecord(projectId: string, id: string) {
  return useQuery({
    queryKey: keys.record(projectId, id),
    queryFn: () => getRecord(projectId, id),
    enabled: id !== '',
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });
}

export function useReplay(projectId: string, id: string) {
  return useMutation({ mutationFn: () => replayRecord(projectId, id) });
}

export function useLogSettings(projectId: string) {
  return useQuery({ queryKey: keys.settings(projectId), queryFn: () => getSettings(projectId) });
}

export function useSaveLogSettings(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (settings: LogSettings) => saveSettings(projectId, settings),
    onSuccess: (result) => {
      if (result.ok) client.setQueryData(keys.settings(projectId), result);
    },
  });
}

export function useLogTokens() {
  return useQuery({ queryKey: keys.tokens, queryFn: listTokens });
}

export function useLogTokensChanged() {
  const client = useQueryClient();
  return () => void client.invalidateQueries({ queryKey: keys.tokens });
}
