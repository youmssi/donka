'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { getSession, signIn, signOut } from './identity.service';
import type { SignInValues } from './schema';

const SESSION_KEY = ['session'] as const;

/** The current session: `data` is the ActionResult of GET /auth/me. */
export function useSession() {
  return useQuery({ queryKey: SESSION_KEY, queryFn: getSession, staleTime: 60_000 });
}

export function useSignIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: SignInValues) => signIn(values),
    onSuccess: (result) => {
      if (result.ok) queryClient.setQueryData(SESSION_KEY, result);
    },
  });
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
