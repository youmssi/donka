import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';
import { routing } from '@/i18n/routing';

export type Account = ApiSchemas['AccountResponse'];
export type AccountList = ApiSchemas['AccountListResponse'];
export type Invitation = ApiSchemas['InvitationRequest'];
export type InvitedUser = ApiSchemas['UserResponse'];

export const inviteSchema = z.object({
  email: z.string().trim().min(1, 'required').email('email'),
  locale: z.enum(routing.locales),
  isAdmin: z.boolean(),
});
export type InviteValues = z.infer<typeof inviteSchema>;
