'use client';

import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  anonymisePerson,
  createRetentionRule,
  endRetentionRule,
  explainRecord,
  findPerson,
  listContests,
  listRetentionRules,
  openContest,
  personReport,
  reviewContest,
  getRecord,
  getSettings,
  listTokens,
  replayRecord,
  saveSettings,
  searchRecords,
} from './decision-log.service';
import type { ExplainLanguage, LogQuery, LogSettingsChange, NewRetentionRule, ReviewValues } from './schema';

const keys = {
  search: (projectId: string, query: LogQuery, offset: number) =>
    ['project', projectId, 'decision-log', 'search', query, offset] as const,
  record: (projectId: string, id: string) => ['project', projectId, 'decision-log', 'record', id] as const,
  settings: (projectId: string) => ['project', projectId, 'decision-log', 'settings'] as const,
  tokens: ['decision-log-tokens'] as const,
  rules: (projectId: string) => ['project', projectId, 'retention-rules'] as const,
  contests: (projectId: string, recordId: string) =>
    ['project', projectId, 'decision-log', 'contests', recordId] as const,
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

export function useExplain(projectId: string, id: string) {
  return useMutation({ mutationFn: (language: ExplainLanguage) => explainRecord(projectId, id, language) });
}

export function useLogSettings(projectId: string) {
  return useQuery({ queryKey: keys.settings(projectId), queryFn: () => getSettings(projectId) });
}

export function useSaveLogSettings(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (settings: LogSettingsChange) => saveSettings(projectId, settings),
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

// ----- People's requests (DNK-75) -------------------------------------------

/** A search is an audited action an owner asks for, not data kept fresh: a mutation. */
export function useFindPerson(projectId: string) {
  return useMutation({ mutationFn: (identifier: string) => findPerson(projectId, identifier) });
}

export function usePersonReport(projectId: string) {
  return useMutation({
    mutationFn: (request: { identifier: string; language: ExplainLanguage; explain: boolean }) =>
      personReport(projectId, request),
  });
}

export function useAnonymisePerson(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (identifier: string) => anonymisePerson(projectId, identifier),
    // Opened records changed: they are read again next time.
    onSuccess: () => void client.invalidateQueries({ queryKey: ['project', projectId, 'decision-log', 'record'] }),
  });
}

export function useRetentionRules(projectId: string) {
  return useQuery({ queryKey: keys.rules(projectId), queryFn: () => listRetentionRules(projectId) });
}

export function useCreateRetentionRule(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (rule: NewRetentionRule) => createRetentionRule(projectId, rule),
    onSettled: () => void client.invalidateQueries({ queryKey: keys.rules(projectId) }),
  });
}

export function useEndRetentionRule(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => endRetentionRule(projectId, id),
    onSettled: () => void client.invalidateQueries({ queryKey: keys.rules(projectId) }),
  });
}

export function useContests(projectId: string, recordId: string) {
  return useQuery({
    queryKey: keys.contests(projectId, recordId),
    queryFn: () => listContests(projectId, recordId),
    enabled: recordId !== '',
  });
}

export function useOpenContest(projectId: string, recordId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (reason: string) => openContest(projectId, recordId, reason),
    onSettled: () => void client.invalidateQueries({ queryKey: keys.contests(projectId, recordId) }),
  });
}

export function useReviewContest(projectId: string, recordId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ id, review }: { id: string; review: ReviewValues }) => reviewContest(projectId, id, review),
    onSettled: () => void client.invalidateQueries({ queryKey: keys.contests(projectId, recordId) }),
  });
}
