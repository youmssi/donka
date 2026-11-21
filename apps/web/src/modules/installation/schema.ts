import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';

export type Installation = ApiSchemas['InstallationResponse'];

/** The server's limit (`donka_installation::MAX_ORGANISATION_CHARS`). */
export const ORGANISATION_MAX = 100;

export const organisationSchema = z.object({
  organisation: z
    .string()
    .trim()
    .min(1, 'required')
    .max(ORGANISATION_MAX, 'maxLength')
    .refine((name) => !/[\u0000-\u001f\u007f]/.test(name), 'oneLine'),
});

export type OrganisationValues = z.infer<typeof organisationSchema>;
