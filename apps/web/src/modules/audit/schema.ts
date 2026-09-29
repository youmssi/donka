import type { ApiSchemas } from '@/components/shared/api';
import openapi from '../../../openapi.json';

export type AuditAction = ApiSchemas['Action'];
export type AuditEvent = ApiSchemas['AuditEventResponse'];
export type AuditList = ApiSchemas['AuditListResponse'];

/** The actions a project's log holds; account events (`user.*`) belong to no project. */
export const PROJECT_ACTIONS = (openapi.components.schemas.Action.enum as AuditAction[]).filter(
  (action) => action.startsWith('project.') || action.startsWith('member.'),
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

const DAY = /^(\d{4})-(\d{2})-(\d{2})$/;

/** Midnight at the start of a day in the viewer's time zone, `days` later. */
function startOfDay(day: string, days = 0): string | undefined {
  if (!DAY.test(day)) return undefined;
  const [year, month, date] = day.split('-').map(Number) as [number, number, number];
  return new Date(year, month - 1, date + days).toISOString();
}

export function toQuery(filters: AuditFilters): AuditQuery {
  return {
    actor: filters.actor,
    action: filters.action,
    from: filters.from ? startOfDay(filters.from) : undefined,
    until: filters.to ? startOfDay(filters.to, 1) : undefined,
  };
}

/** Reads the filters from the page's address, dropping values the API would refuse. */
export function readFilters(params: URLSearchParams): AuditFilters {
  const action = params.get('action') as AuditAction | null;
  const day = (name: string) => {
    const value = params.get(name);
    return value && DAY.test(value) ? value : undefined;
  };
  return {
    actor: params.get('actor') || undefined,
    action: action && PROJECT_ACTIONS.includes(action) ? action : undefined,
    from: day('from'),
    to: day('to'),
  };
}

/** True when the last day comes before the first: nothing could match. */
export function isEmptyRange(filters: AuditFilters): boolean {
  return Boolean(filters.from && filters.to && filters.to < filters.from);
}

/** A text field of an event's details, or '' when absent. */
export function detail(event: AuditEvent, ...path: string[]): string {
  let node: unknown = event.details;
  for (const key of path) {
    node = node && typeof node === 'object' ? (node as Record<string, unknown>)[key] : undefined;
  }
  return typeof node === 'string' ? node : '';
}
