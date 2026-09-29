'use client';

import { CircleCheck } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Pager } from '@/components/shared/pager';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import { InviteDialog } from './invite-dialog';
import { PAGE_SIZE } from './people.service';
import type { Account } from './schema';
import { useAccounts, useInvite } from './usePeople';

/** Every Studio account (administrators only; others see why not). */
export function PeoplePage() {
  const t = useTranslations('people');
  const common = useTranslations('common');
  const offset = Math.max(0, Number(useSearchParams().get('offset')) || 0);
  const accounts = useAccounts(offset);
  const result = accounts.data;
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<ActionError | null>(null);

  if (result && !result.ok && result.error.code === 'FORBIDDEN') {
    return <ErrorAlert error={result.error} title={t('forbiddenTitle')} />;
  }

  return (
    <div className="grid gap-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1">
          <h1 className="text-2xl font-semibold tracking-tight">{t('title')}</h1>
          <p className="text-sm text-muted-foreground">{t('description')}</p>
        </div>
        <InviteDialog
          onInvited={(email) => {
            setError(null);
            setNotice(t('invited', { email }));
          }}
        />
      </div>
      {notice ? (
        <Alert variant="success" aria-live="polite">
          <CircleCheck aria-hidden />
          <AlertDescription>{notice}</AlertDescription>
        </Alert>
      ) : null}
      {error ? <ErrorAlert error={error} /> : null}

      {!result ? (
        <PageSkeleton />
      ) : !result.ok ? (
        <ErrorAlert
          error={result.error}
          title={t('errorTitle')}
          action={
            <Button variant="outline" size="sm" className="mt-2" onClick={() => void accounts.refetch()}>
              {common('retry')}
            </Button>
          }
        />
      ) : (
        <>
          <ul className="divide-y rounded-xl border bg-card">
            {result.data.items.map((account) => (
              <AccountRow
                key={account.id}
                account={account}
                onResent={(email) => {
                  setError(null);
                  setNotice(t('resent', { email }));
                }}
                onError={(failure) => {
                  setNotice(null);
                  setError(failure);
                }}
              />
            ))}
          </ul>
          {result.data.total > PAGE_SIZE ? (
            <Pager
              hrefFor={(next) => `/people?offset=${next}`}
              offset={offset}
              pageSize={PAGE_SIZE}
              total={result.data.total}
            />
          ) : null}
        </>
      )}
    </div>
  );
}

function AccountRow({
  account,
  onResent,
  onError,
}: {
  account: Account;
  onResent: (email: string) => void;
  onError: (error: ActionError) => void;
}) {
  const t = useTranslations('people');
  const invite = useInvite();

  async function resend() {
    const result = await invite.mutateAsync({
      email: account.email,
      locale: account.locale,
      isAdmin: account.isAdmin,
    });
    if (result.ok) onResent(account.email);
    else onError(result.error);
  }

  return (
    <li className="flex flex-wrap items-center gap-2 px-4 py-3">
      <span className="min-w-0 flex-1 truncate">{account.email}</span>
      {account.isAdmin ? <Badge>{t('admin')}</Badge> : null}
      <Badge variant={account.active ? 'secondary' : 'outline'}>{account.active ? t('active') : t('pending')}</Badge>
      {account.active ? null : (
        <Button
          variant="ghost"
          size="sm"
          disabled={invite.isPending}
          aria-label={t('resendFor', { email: account.email })}
          onClick={() => void resend()}
        >
          {invite.isPending ? t('sending') : t('resend')}
        </Button>
      )}
    </li>
  );
}
