import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { AccountList, InviteValues, InvitedUser } from './schema';

export const PAGE_SIZE = 50;

export function listAccounts(offset: number): Promise<ActionResult<AccountList>> {
  return attempt(async () => {
    const response = await http.get('users', { searchParams: { limit: PAGE_SIZE, offset } });
    return response.ok ? success(await response.json<AccountList>()) : failure(response);
  });
}

/** Invites a person; for someone who has not accepted yet, sends a fresh link. */
export function invite(values: InviteValues): Promise<ActionResult<InvitedUser>> {
  return attempt(async () => {
    const response = await http.post('users/invitations', { json: values });
    return response.ok ? success(await response.json<InvitedUser>()) : failure(response);
  });
}
