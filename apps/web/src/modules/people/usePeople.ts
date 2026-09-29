'use client';

import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { invite, listAccounts } from './people.service';
import type { InviteValues } from './schema';

const ACCOUNTS = ['accounts'] as const;

export function useAccounts(offset: number) {
  return useQuery({
    queryKey: [...ACCOUNTS, offset],
    queryFn: () => listAccounts(offset),
    placeholderData: keepPreviousData,
  });
}

export function useInvite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: InviteValues) => invite(values),
    onSuccess: (result) => {
      if (result.ok) void queryClient.invalidateQueries({ queryKey: ACCOUNTS });
    },
  });
}
