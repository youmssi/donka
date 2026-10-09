'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { getOnboarding, listToursSeen, markTourSeen } from './onboarding.service';
import type { Tour } from './schema';

const keys = { checklist: ['onboarding'] as const, tours: ['me', 'tours'] as const };

export function useOnboarding() {
  // Read on each visit: steps are done on other pages.
  return useQuery({ queryKey: keys.checklist, queryFn: getOnboarding, staleTime: 0 });
}

export function useToursSeen() {
  return useQuery({ queryKey: keys.tours, queryFn: listToursSeen, staleTime: Infinity });
}

export function useMarkTourSeen() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tour: Tour) => markTourSeen(tour),
    // Seen at once on this device, whatever the server answers: a tour never restarts mid-visit.
    onMutate: (tour) =>
      queryClient.setQueryData(keys.tours, (previous: Awaited<ReturnType<typeof listToursSeen>> | undefined) =>
        previous?.ok && !previous.data.includes(tour) ? { ...previous, data: [...previous.data, tour] } : previous,
      ),
  });
}
