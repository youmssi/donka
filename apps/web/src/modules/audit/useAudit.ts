'use client';

import { keepPreviousData, useQuery } from '@tanstack/react-query';

import { listAudit } from './audit.service';
import type { AuditQuery } from './schema';

export function useAuditLog(projectId: string, query: AuditQuery, offset: number, enabled: boolean) {
  return useQuery({
    queryKey: ['project', projectId, 'audit', query, offset],
    queryFn: () => listAudit(projectId, query, offset),
    placeholderData: keepPreviousData,
    enabled,
  });
}
