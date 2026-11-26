'use client';

import { useMutation, useQuery } from '@tanstack/react-query';

import type { NewProjectValues } from '@/modules/project';
import { useRefreshProjects } from '@/modules/project';

import { duplicateProject, exportProject, importPack, inspectPack, listPacks } from './pack.service';

export function usePacks(enabled: boolean) {
  // The catalogue is read once when Studio starts: it does not change while it runs.
  return useQuery({ queryKey: ['packs'], queryFn: listPacks, enabled, staleTime: Infinity });
}

export function useInspectPack() {
  return useMutation({ mutationFn: (file: Blob) => inspectPack(file) });
}

export function useImportPack() {
  const refresh = useRefreshProjects();
  return useMutation({
    mutationFn: ({ pack, values }: { pack: string | Blob; values: NewProjectValues }) => importPack(pack, values),
    onSuccess: (result) => {
      if (result.ok) refresh();
    },
  });
}

export function useDuplicateProject(projectId: string) {
  const refresh = useRefreshProjects();
  return useMutation({
    mutationFn: ({ values, releaseId }: { values: NewProjectValues; releaseId?: string }) =>
      duplicateProject(projectId, values, releaseId),
    onSuccess: (result) => {
      if (result.ok) refresh();
    },
  });
}

export function useExportProject(projectId: string) {
  return useMutation({ mutationFn: (releaseId?: string) => exportProject(projectId, releaseId) });
}
