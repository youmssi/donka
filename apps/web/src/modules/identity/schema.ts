import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';
import openapi from '../../../openapi.json';

export type User = ApiSchemas['UserResponse'];

// Password limits come from the API contract, so the form and the server never disagree.
const passwordRules = openapi.components.schemas.PasswordSetupRequest.properties.password;
export const PASSWORD_MIN = passwordRules.minLength;
export const PASSWORD_MAX = passwordRules.maxLength;

/** Zod issues carry a message key under `validation`, translated by the form field. */
const email = z.string().trim().min(1, 'required').email('email');

export const signInSchema = z.object({
  email,
  password: z.string().min(1, 'required'),
});
export type SignInValues = z.infer<typeof signInSchema>;

export const forgotPasswordSchema = z.object({ email });
export type ForgotPasswordValues = z.infer<typeof forgotPasswordSchema>;

export const setupPasswordSchema = z
  .object({
    password: z.string().min(PASSWORD_MIN, 'passwordLength').max(PASSWORD_MAX, 'passwordLength'),
    confirm: z.string().min(1, 'required'),
  })
  .refine((values) => values.password === values.confirm, { path: ['confirm'], message: 'passwordMismatch' });
export type SetupPasswordValues = z.infer<typeof setupPasswordSchema>;
