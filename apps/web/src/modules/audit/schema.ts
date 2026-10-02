import type { ApiSchemas } from '@/components/shared/api';
import { isEmptyRange as emptyRange, rangeQuery, readDay } from '@/components/shared/date-range';
import openapi from '../../../openapi.json';

export type AuditAction = ApiSchemas['Action'];
export type AuditEvent = ApiSchemas['AuditEventResponse'];
export type AuditList = ApiSchemas['AuditListResponse'];

/** The actions a project's log holds; account and decision-log token events belong to no project. */
export const PROJECT_ACTIONS = (openapi.components.schemas.Action.enum as AuditAction[]).filter(
  (action) => !action.startsWith('user.') && !action.startsWith('decision_log_token.'),
);

/** The filters as the page's address holds them: dates are days in the viewer's time zone. */
export interface AuditFilters {
  actor?: string;
  action?: AuditAction;
  /** First day shown, `YYYY-MM-DD`. */
  from?: string;
  /** Last day shown, `YYYY-MM-DD`, inclusive. */
  to?: string;
}

/** The API query for these filters: `from` and `until` are instants, `until` exclusive. */
export interface AuditQuery {
  actor?: string;
  action?: AuditAction;
  from?: string;
  until?: string;
}

export function toQuery(filters: AuditFilters): AuditQuery {
  return { actor: filters.actor, action: filters.action, ...rangeQuery(filters.from, filters.to) };
}

/** Reads the filters from the page's address, dropping values the API would refuse. */
export function readFilters(params: URLSearchParams): AuditFilters {
  const action = params.get('action') as AuditAction | null;
  return {
    actor: params.get('actor') || undefined,
    action: action && PROJECT_ACTIONS.includes(action) ? action : undefined,
    from: readDay(params.get('from')),
    to: readDay(params.get('to')),
  };
}

/** True when the last day comes before the first: nothing could match. */
export function isEmptyRange(filters: AuditFilters): boolean {
  return emptyRange(filters.from, filters.to);
}

/** A text or number field of an event's details, as text, or '' when absent. */
export function detail(event: AuditEvent, ...path: string[]): string {
  let node: unknown = event.details;
  for (const key of path) {
    node = node && typeof node === 'object' ? (node as Record<string, unknown>)[key] : undefined;
  }
  return typeof node === 'string' ? node : typeof node === 'number' ? String(node) : '';
}
