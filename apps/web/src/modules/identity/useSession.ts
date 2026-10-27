'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  completeSignIn,
  confirmEnrolment,
  confirmSignInEnrolment,
  disableSecondStep,
  getSecondStep,
  getSession,
  regenerateRecoveryCodes,
  signIn,
  signOut,
  startSignInEnrolment,
} from './identity.service';
import type { CodeValues, SignInValues, User } from './schema';

const SESSION_KEY = ['session'] as const;

/** The current session: `data` is the ActionResult of GET /auth/me. */
export function useSession() {
  return useQuery({ queryKey: SESSION_KEY, queryFn: getSession, staleTime: 60_000 });
}

const SECOND_STEP_KEY = ['second-step'] as const;

/** Signs in; the session is cached only when it opened (no second step). */
export function useSignIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: SignInValues) => signIn(values),
    onSuccess: (result) => {
      if (result.ok && result.data.step === 'signedIn') signedIn(queryClient, result.data.user);
    },
  });
}

export function useCompleteSignIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: CodeValues) => completeSignIn(values),
    onSuccess: (result) => {
      if (result.ok) signedIn(queryClient, result.data);
    },
  });
}

export function useStartSignInEnrolment() {
  return useMutation({ mutationFn: () => startSignInEnrolment() });
}

/** The session opens on success; the caller shows the recovery codes before moving on. */
export function useConfirmSignInEnrolment() {
  return useMutation({ mutationFn: (values: CodeValues) => confirmSignInEnrolment(values) });
}

/** Called once the person has seen their recovery codes. */
export function useFinishSignIn() {
  const queryClient = useQueryClient();
  return (user: User) => signedIn(queryClient, user);
}

function signedIn(queryClient: ReturnType<typeof useQueryClient>, user: User) {
  queryClient.setQueryData(SESSION_KEY, { ok: true, data: user });
}

export function useSecondStep() {
  return useQuery({ queryKey: SECOND_STEP_KEY, queryFn: getSecondStep });
}

/** Mutations that change the second step refresh its status. */
function useSecondStepMutation<T>(fn: (values: CodeValues) => Promise<T>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: CodeValues) => fn(values),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: SECOND_STEP_KEY }),
  });
}

export function useConfirmEnrolment() {
  return useSecondStepMutation(confirmEnrolment);
}

export function useRegenerateRecoveryCodes() {
  return useSecondStepMutation(regenerateRecoveryCodes);
}

export function useDisableSecondStep() {
  return useSecondStepMutation(disableSecondStep);
}

export function useSignOut() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: signOut,
    onSuccess: (result) => {
      // Forget everything cached for the previous user.
      if (result.ok) queryClient.clear();
    },
  });
}
