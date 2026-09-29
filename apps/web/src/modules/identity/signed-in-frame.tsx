'use client';

import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { createContext, useContext, useEffect, type ReactNode } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { AppShell } from '@/components/shared/layout/app-shell';
import { MainNav } from '@/components/shared/layout/main-nav';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Button } from '@/components/ui/button';
import { usePathname, useRouter } from '@/i18n/navigation';

import type { User } from './schema';
import { UserMenu } from './user-menu';
import { useSession } from './useSession';

const CurrentUserContext = createContext<User | null>(null);

/** The signed-in user. Only valid inside SignedInFrame. */
export function useCurrentUser(): User {
  const user = useContext(CurrentUserContext);
  if (!user) throw new Error('useCurrentUser is used outside SignedInFrame');
  return user;
}

/**
 * Frame of every signed-in page: sends visitors without a session to sign-in
 * (and back here afterwards), then shows the shell with the account menu.
 */
export function SignedInFrame({ children }: { children: ReactNode }) {
  const t = useTranslations();
  const router = useRouter();
  const pathname = usePathname();
  const query = useSearchParams().toString();
  const session = useSession();
  const result = session.data;
  const signedOut = result?.ok === true && result.data === null;

  useEffect(() => {
    if (!signedOut) return;
    const here = query ? `${pathname}?${query}` : pathname;
    router.replace(`/sign-in?next=${encodeURIComponent(here)}`);
  }, [signedOut, pathname, query, router]);

  if (!result) return <Loading />;
  if (!result.ok) {
    return (
      <AppShell>
        <ErrorAlert
          error={result.error}
          title={t('session.errorTitle')}
          action={
            <Button variant="outline" size="sm" className="mt-2" onClick={() => void session.refetch()}>
              {t('common.retry')}
            </Button>
          }
        />
      </AppShell>
    );
  }
  // No session: the effect above is sending the visitor to sign-in.
  if (!result.data) return <Loading />;
  return (
    <CurrentUserContext value={result.data}>
      <AppShell nav={<MainNav isAdmin={result.data.isAdmin} />} account={<UserMenu user={result.data} />}>
        {children}
      </AppShell>
    </CurrentUserContext>
  );
}

function Loading() {
  return (
    <AppShell>
      <PageSkeleton />
    </AppShell>
  );
}
