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
