import { API_BASE, attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { AuditList, AuditQuery } from './schema';

export const PAGE_SIZE = 50;

function searchParams(query: AuditQuery): URLSearchParams {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(query)) if (value) params.set(name, value);
  return params;
}

export function listAudit(projectId: string, query: AuditQuery, offset: number): Promise<ActionResult<AuditList>> {
  return attempt(async () => {
    const params = searchParams(query);
    params.set('limit', String(PAGE_SIZE));
    params.set('offset', String(offset));
    const response = await http.get(`projects/${encodeURIComponent(projectId)}/audit`, { searchParams: params });
    return response.ok ? success(await response.json<AuditList>()) : failure(response);
  });
}

/** The CSV download of the same events; the browser sends the session cookie itself. */
export function exportHref(projectId: string, query: AuditQuery): string {
  const params = searchParams(query).toString();
  return `${API_BASE}/projects/${encodeURIComponent(projectId)}/audit/export${params ? `?${params}` : ''}`;
}
