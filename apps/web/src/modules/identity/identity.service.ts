import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { ForgotPasswordValues, SignInValues, User } from './schema';

/** The signed-in user, or null when there is no valid session. */
export function getSession(): Promise<ActionResult<User | null>> {
  return attempt(async () => {
    const response = await http.get('auth/me');
    if (response.ok) return success(await response.json<User>());
    if (response.status === 401) return success(null);
    return failure(response);
  });
}

export function signIn(values: SignInValues): Promise<ActionResult<User>> {
  return attempt(async () => {
    const response = await http.post('auth/sign-in', { json: values });
    return response.ok ? success(await response.json<User>()) : failure(response);
  });
}

export function signOut(): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.post('auth/sign-out');
    // Already signed out is the outcome the person asked for.
    return response.ok || response.status === 401 ? success(null) : failure(response);
  });
}

export function requestPasswordReset(values: ForgotPasswordValues): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.post('auth/password-reset', { json: values });
    return response.ok ? success(null) : failure(response);
  });
}

export function setPassword(token: string, password: string): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.post('auth/password-setup', { json: { token, password } });
    return response.ok ? success(null) : failure(response);
  });
}
