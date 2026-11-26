import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  CodeValues,
  Enrolment,
  ForgotPasswordValues,
  SecondStepStatus,
  SignInOutcome,
  SignInValues,
  User,
} from './schema';

/** The signed-in user, or null when there is no valid session. */
export function getSession(): Promise<ActionResult<User | null>> {
  return attempt(async () => {
    const response = await http.get('auth/me');
    if (response.ok) return success(await response.json<User>());
    if (response.status === 401) return success(null);
    return failure(response);
  });
}

/** 200 opens the session; 202 says which second step comes next (DNK-59). */
export function signIn(values: SignInValues): Promise<ActionResult<SignInOutcome>> {
  return attempt(async () => {
    const response = await http.post('auth/sign-in', { json: values });
    if (response.status === 202) {
      const { next } = await response.json<{ next: 'code' | 'enrol' }>();
      return success<SignInOutcome>({ step: next });
    }
    return response.ok
      ? success<SignInOutcome>({ step: 'signedIn', user: await response.json<User>() })
      : failure(response);
  });
}

/** Finishes a sign-in with an authenticator code or a recovery code. */
export function completeSignIn(values: CodeValues): Promise<ActionResult<User>> {
  return attempt(async () => {
    const response = await http.post('auth/sign-in/code', { json: values });
    return response.ok ? success(await response.json<User>()) : failure(response);
  });
}

/** Starts the setup policy requires before the sign-in can finish. */
export function startSignInEnrolment(): Promise<ActionResult<Enrolment>> {
  return attempt(async () => {
    const response = await http.post('auth/sign-in/enrolment');
    return response.ok ? success(await response.json<Enrolment>()) : failure(response);
  });
}

export function confirmSignInEnrolment(
  values: CodeValues,
): Promise<ActionResult<{ user: User; recoveryCodes: string[] }>> {
  return attempt(async () => {
    const response = await http.post('auth/sign-in/enrolment/confirm', { json: values });
    return response.ok ? success(await response.json<{ user: User; recoveryCodes: string[] }>()) : failure(response);
  });
}

export function getSecondStep(): Promise<ActionResult<SecondStepStatus>> {
  return attempt(async () => {
    const response = await http.get('account/second-step');
    return response.ok ? success(await response.json<SecondStepStatus>()) : failure(response);
  });
}

export function startEnrolment(): Promise<ActionResult<Enrolment>> {
  return attempt(async () => {
    const response = await http.post('account/second-step/enrolment');
    return response.ok ? success(await response.json<Enrolment>()) : failure(response);
  });
}

/** Turns the second step on; returns the recovery codes, shown once. */
export function confirmEnrolment(values: CodeValues): Promise<ActionResult<string[]>> {
  return recoveryCodes('account/second-step/enrolment/confirm', values);
}

export function regenerateRecoveryCodes(values: CodeValues): Promise<ActionResult<string[]>> {
  return recoveryCodes('account/second-step/recovery-codes', values);
}

function recoveryCodes(path: string, values: CodeValues): Promise<ActionResult<string[]>> {
  return attempt(async () => {
    const response = await http.post(path, { json: values });
    if (!response.ok) return failure(response);
    const body = await response.json<{ recoveryCodes: string[] }>();
    return success(body.recoveryCodes);
  });
}

export function disableSecondStep(values: CodeValues): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.post('account/second-step/disable', { json: values });
    return response.ok ? success(null) : failure(response);
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
