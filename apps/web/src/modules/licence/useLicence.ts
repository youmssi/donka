'use client';

import { useQuery } from '@tanstack/react-query';

import { getLicence } from './licence.service';
import type { LicensedModule } from './schema';

/** The installation's licence. It changes when an operator reloads the file, so it is kept a while. */
export function useLicence() {
  return useQuery({ queryKey: ['licence'], queryFn: getLicence, staleTime: 5 * 60_000 });
}

/**
 * Whether the licence offers `module`. While the licence loads, or if it cannot be read, the
 * module is offered: the server refuses what the licence leaves out, with a clear error.
 */
export function useModule(module: LicensedModule): boolean {
  const result = useLicence().data;
  return result?.ok ? result.data.modules.includes(module) : true;
}
