'use client';

import { useQuery } from '@tanstack/react-query';

import { getUpdates } from './updates.service';

/** Studio checks for new releases once a day, so an hour-old answer is fresh enough. */
export function useUpdates(isAdmin: boolean) {
  return useQuery({
    queryKey: ['updates'],
    queryFn: getUpdates,
    enabled: isAdmin,
    staleTime: 60 * 60_000,
  });
}
