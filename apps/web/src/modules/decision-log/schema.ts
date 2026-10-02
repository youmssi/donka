import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';
import { isEmptyRange as emptyRange, rangeQuery, readDay } from '@/components/shared/date-range';
import openapi from '../../../openapi.json';

export type RecordSummary = ApiSchemas['RecordSummaryResponse'];
export type RecordList = ApiSchemas['RecordListResponse'];
export type DecisionRecord = ApiSchemas['RecordResponse'];
export type Replay = ApiSchemas['ReplayResponse'];
export type RecordStatus = ApiSchemas['RecordStatus'];
export type EnvironmentName = ApiSchemas['EnvironmentName'];
export type LogSettings = ApiSchemas['DecisionLogSettingsBody'];
export type LogToken = ApiSchemas['LogTokenResponse'];
export type IssuedLogToken = ApiSchemas['IssuedLogTokenResponse'];

// Limits come from the API contract, so the forms and the server never disagree.
export const TOKEN_NAME_MAX = openapi.components.schemas.IssueLogTokenRequest.properties.name.maxLength;
export const OUTCOME_FIELD_MAX = 200;

const ENVIRONMENTS: readonly EnvironmentName[] = ['production', 'staging'];
const STATUSES: readonly RecordStatus[] = ['succeeded', 'failed'];

/** The filters as the page's address holds them: dates are days in the viewer's time zone. */
export interface LogFilters {
  reference?: string;
  decision?: string;
  outcome?: string;
  environment?: EnvironmentName;
  status?: RecordStatus;
  /** First day shown, `YYYY-MM-DD`. */
  from?: string;
  /** Last day shown, `YYYY-MM-DD`, inclusive. */
  to?: string;
}

/** The API query for these filters. */
export interface LogQuery {
  reference?: string;
  decisionKey?: string;
  outcome?: string;
  environment?: EnvironmentName;
  status?: RecordStatus;
  from?: string;
  until?: string;
}

export function toQuery(filters: LogFilters): LogQuery {
  return {
    reference: filters.reference,
    decisionKey: filters.decision,
    outcome: filters.outcome,
    environment: filters.environment,
    status: filters.status,
    ...rangeQuery(filters.from, filters.to),
  };
}

/** Reads the filters from the page's address, dropping values the API would refuse. */
export function readFilters(params: URLSearchParams): LogFilters {
  const text = (name: string) => {
    const value = params.get(name)?.trim();
    return value && value.length <= OUTCOME_FIELD_MAX ? value : undefined;
  };
  const oneOf = <T extends string>(name: string, values: readonly T[]) => {
    const value = params.get(name) as T | null;
    return value && values.includes(value) ? value : undefined;
  };
  return {
    reference: text('reference'),
    decision: text('decision'),
    outcome: text('outcome'),
    environment: oneOf('environment', ENVIRONMENTS),
    status: oneOf('status', STATUSES),
    from: readDay(params.get('from')),
    to: readDay(params.get('to')),
  };
}

export function isFiltered(filters: LogFilters): boolean {
  return Object.values(filters).some(Boolean);
}

/** True when the last day comes before the first: nothing could match. */
export function isEmptyRange(filters: LogFilters): boolean {
  return emptyRange(filters.from, filters.to);
}

/** How long the Runtime took: microseconds below a millisecond, else milliseconds. */
export function duration(us: number, locale: string): string {
  if (us < 1000) return `${new Intl.NumberFormat(locale).format(us)} µs`;
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(us / 1000)} ms`;
}

/** The feed address a Runtime sends to, from where Studio is served. */
export function feedUrl(origin: string): string {
  return `${origin}/api/v1/decision-log/records`;
}

// `decision`, `result.band`: field names joined by dots (the API checks the same).
const FIELD_PATH = /^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$/;

export const settingsSchema = z.object({
  outcomeField: z
    .string()
    .trim()
    .max(OUTCOME_FIELD_MAX, 'maxLength')
    .refine((value) => value === '' || FIELD_PATH.test(value), 'fieldPath'),
});
export type SettingsValues = z.infer<typeof settingsSchema>;

export const tokenSchema = z.object({
  environment: z.enum(['production', 'staging']),
  name: z.string().trim().min(1, 'required').max(TOKEN_NAME_MAX, 'maxLength'),
});
export type TokenValues = z.infer<typeof tokenSchema>;
