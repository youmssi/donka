import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';
import openapi from '../../../openapi.json';

export type DecisionSummary = ApiSchemas['DecisionSummaryResponse'];
export type Decision = ApiSchemas['DecisionResponse'];
export type SimulationResult = ApiSchemas['SimulateResponse'];
export type Version = ApiSchemas['DecisionVersionResponse'];
export type VersionDetail = ApiSchemas['DecisionVersionDetailResponse'];
export type VersionList = ApiSchemas['DecisionVersionListResponse'];
export type Restored = ApiSchemas['RestoreResponse'];
export type Scenario = ApiSchemas['ScenarioResponse'];
export type ScenarioList = ApiSchemas['ScenarioListResponse'];
export type MatchMode = Scenario['match'];
export type TestSummary = ApiSchemas['TestSummaryResponse'];
export type TestResult = ApiSchemas['TestResultResponse'];
export type TestResultList = ApiSchemas['TestResultListResponse'];

// The message rule comes from the API contract too.
const messageField = openapi.components.schemas.SaveVersionRequest.properties.message;
export const MESSAGE_MAX = messageField.maxLength;

export const saveVersionSchema = z.object({
  message: z.string().trim().min(1, 'required').max(MESSAGE_MAX, 'maxLength'),
});
export type SaveVersionValues = z.infer<typeof saveVersionSchema>;

// The key rule comes from the API contract, so the form and the server never disagree.
const keyField = openapi.components.schemas.CreateDecisionRequest.properties.key;
export const KEY_RULES = { max: keyField.maxLength, pattern: new RegExp(keyField.pattern) };

export const createDecisionSchema = z.object({
  key: z
    .string()
    .trim()
    .min(1, 'required')
    .refine((key) => key.length <= KEY_RULES.max && KEY_RULES.pattern.test(key), 'decisionKey'),
});
export type CreateDecisionValues = z.infer<typeof createDecisionSchema>;

export const SCENARIO_NAME_MAX = openapi.components.schemas.CreateScenarioRequest.properties.name.maxLength;

/** Text that parses as a JSON object, as scenarios take their input and expected output. */
export function parseObject(text: string): Record<string, unknown> | null {
  try {
    const value: unknown = JSON.parse(text);
    return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

const jsonObject = z.string().refine((text) => parseObject(text) !== null, 'jsonObject');

/** A scenario as the form edits it: input and expected output as JSON text. */
export const scenarioSchema = z.object({
  decisionId: z.string().min(1, 'required'),
  name: z.string().trim().min(1, 'required').max(SCENARIO_NAME_MAX, 'maxLength'),
  input: jsonObject,
  expected: jsonObject,
  match: z.enum(['exact', 'partial']),
});
export type ScenarioValues = z.infer<typeof scenarioSchema>;

/** `bureau/normalize` → `{ folder: 'bureau/', name: 'normalize' }`. */
export function splitKey(key: string): { folder: string; name: string } {
  const at = key.lastIndexOf('/');
  return at < 0 ? { folder: '', name: key } : { folder: key.slice(0, at + 1), name: key.slice(at + 1) };
}

/** Who saved first, from a `DECISION_CONFLICT` error. */
export interface Conflict {
  revision: number;
  updatedAt: string;
  updatedBy: { id: string; email: string } | null;
}

export function conflictOf(details: unknown): Conflict | null {
  if (!details || typeof details !== 'object' || !('revision' in details)) return null;
  const { revision, updatedAt, updatedBy } = details as Record<string, unknown>;
  if (typeof revision !== 'number' || typeof updatedAt !== 'string') return null;
  const by =
    updatedBy && typeof updatedBy === 'object' && 'email' in updatedBy
      ? (updatedBy as { id: string; email: string })
      : null;
  return { revision, updatedAt, updatedBy: by };
}
