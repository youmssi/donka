'use client';

import { useCallback, type ReactNode } from 'react';

import { TourProvider } from '@/components/shared/tour';

import type { Tour } from './schema';
import { useMarkTourSeen, useToursSeen } from './useOnboarding';

const TOURS: readonly Tour[] = ['editor', 'releases', 'environments'];

function isTour(id: string): id is Tour {
  return TOURS.some((tour) => tour === id);
}

/** Tours start once per person: what they have seen is kept on the server. */
export function TourState({ children }: { children: ReactNode }) {
  const seen = useToursSeen().data;
  const { mutate } = useMarkTourSeen();
  const onSeen = useCallback(
    (id: string) => {
      if (isTour(id)) mutate(id);
    },
    [mutate],
  );
  // Unknown (loading, or the server did not answer): no tour starts on its own.
  return (
    <TourProvider seen={seen?.ok ? seen.data : undefined} onSeen={onSeen}>
      {children}
    </TourProvider>
  );
}
