'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ActionResult } from '@/components/shared/api';

import { finishFirstRun, getInstallation, renameOrganisation } from './installation.service';
import type { Installation, OrganisationValues } from './schema';

const KEY = ['installation'];

/** The installation: first run or not, and its organisation. It rarely changes. */
export function useInstallation() {
  return useQuery({ queryKey: KEY, queryFn: getInstallation, staleTime: 5 * 60_000 });
}

/** The organisation's name; `null` while it loads, when it is not named, or cannot be read. */
export function useOrganisation(): string | null {
  const result = useInstallation().data;
  return result?.ok ? (result.data.organisation ?? null) : null;
}

function useSaveInstallation(save: (values: OrganisationValues) => Promise<ActionResult<Installation>>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (values: OrganisationValues) => save(values),
    onSuccess: (result) => {
      if (result.ok) client.setQueryData<ActionResult<Installation>>(KEY, result);
    },
  });
}

export function useFinishFirstRun() {
  return useSaveInstallation(finishFirstRun);
}

export function useRenameOrganisation() {
  return useSaveInstallation(renameOrganisation);
}
