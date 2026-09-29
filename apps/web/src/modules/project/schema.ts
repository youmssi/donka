import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';
import openapi from '../../../openapi.json';

export type Role = ApiSchemas['Role'];
export type Project = ApiSchemas['ProjectResponse'];
export type ProjectSummary = ApiSchemas['ProjectSummaryResponse'];
export type ProjectList = ApiSchemas['ProjectListResponse'];
export type Member = ApiSchemas['MemberResponse'];

/** Roles from the API contract, most powerful first. */
export const ROLES = openapi.components.schemas.Role.enum.slice().reverse() as Role[];

// Field rules come from the API contract, so the forms and the server never disagree.
const fields = openapi.components.schemas.CreateProjectRequest.properties;
export const KEY_RULES = {
  min: fields.key.minLength,
  max: fields.key.maxLength,
  pattern: new RegExp(fields.key.pattern),
};
export const NAME_MAX = fields.name.maxLength;
export const DESCRIPTION_MAX = fields.description.maxLength;

/** Zod issues carry a message key under `validation`, translated by the form field. */
export const projectDetailsSchema = z.object({
  name: z.string().trim().min(1, 'required').max(NAME_MAX, 'maxLength'),
  description: z.string().trim().max(DESCRIPTION_MAX, 'maxLength'),
});
export type ProjectDetailsValues = z.infer<typeof projectDetailsSchema>;

export const createProjectSchema = projectDetailsSchema.extend({
  key: z
    .string()
    .min(1, 'required')
    .refine(
      (key) => key.length >= KEY_RULES.min && key.length <= KEY_RULES.max && KEY_RULES.pattern.test(key),
      'projectKey',
    ),
});
export type CreateProjectValues = z.infer<typeof createProjectSchema>;

export const addMemberSchema = z.object({
  email: z.string().trim().min(1, 'required').email('email'),
  role: z.enum(['owner', 'editor', 'viewer'] satisfies Role[]),
});
export type AddMemberValues = z.infer<typeof addMemberSchema>;

/**
 * A key proposed from the name: "Crédit PME 2026" becomes "credit-pme-2026". The
 * person can still edit it; it must follow the same rule as any key.
 */
export function keyFromName(name: string): string {
  return name
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^[^a-z]+/, '')
    .slice(0, KEY_RULES.max)
    .replace(/-+$/, '');
}
