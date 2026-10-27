'use client';

import { MailPlus, ShieldOff, Users } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { toast } from 'sonner';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { PageHeader } from '@/components/shared/layout/page-header';
import { Person } from '@/components/shared/person';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Spinner } from '@/components/ui/spinner';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { useCurrentUser } from '@/modules/identity';

import { InviteDialog } from './invite-dialog';
import { PAGE_SIZE } from './people.service';
import type { Account } from './schema';
import { useAccounts, useInvite, useResetSecondStep } from './usePeople';

/** Every Studio account (administrators only; others see why not). */
export function PeoplePage() {
  const t = useTranslations('people');
  const common = useTranslations('common');
  const offset = Math.max(0, Number(useSearchParams().get('offset')) || 0);
  const accounts = useAccounts(offset);
  const result = accounts.data;

  if (result && !result.ok && result.error.code === 'FORBIDDEN') {
    return <ErrorAlert error={result.error} title={t('forbiddenTitle')} />;
  }

  const columns: DataTableColumn<Account>[] = [
    {
      id: 'person',
      header: t('person'),
      cell: ({ row }) => <Person email={row.original.email} />,
      meta: { className: 'w-full max-w-0' },
    },
    {
      id: 'access',
      header: t('access'),
      cell: ({ row }) =>
        row.original.isAdmin ? (
          <Badge>{t('admin')}</Badge>
        ) : (
          <span className="text-muted-foreground">{t('member')}</span>
        ),
      meta: { className: 'hidden sm:table-cell' },
    },
    {
      id: 'status',
      header: t('status'),
      cell: ({ row }) => (
        <Badge variant={row.original.active ? 'secondary' : 'outline'}>
          {row.original.active ? t('active') : t('pending')}
        </Badge>
      ),
    },
    {
      id: 'secondStep',
      header: t('secondStep'),
      cell: ({ row }) =>
        row.original.secondStep ? (
          <Badge variant="secondary">{t('secondStepOn')}</Badge>
        ) : (
          <span className="text-muted-foreground">{t('secondStepOff')}</span>
        ),
      meta: { className: 'hidden sm:table-cell' },
    },
    {
      id: 'since',
      header: t('since'),
      cell: ({ row }) => <When value={row.original.createdAt} />,
      meta: { className: 'hidden text-muted-foreground md:table-cell' },
    },
    {
      id: 'actions',
      header: () => <span className="sr-only">{common('actions')}</span>,
      cell: ({ row }) =>
        row.original.active ? (
          row.original.secondStep ? (
            <ResetSecondStep account={row.original} />
          ) : null
        ) : (
          <Resend account={row.original} />
        ),
      meta: { className: 'w-12 text-right' },
    },
  ];

  return (
    <div className="grid gap-4">
      <PageHeader title={t('title')} description={t('description')} actions={<InviteDialog />} />
      {result && !result.ok ? (
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
        <DataTable
          label={t('title')}
          columns={columns}
          data={result?.data.items}
          getRowId={(account) => account.id}
          pagination={
            result
              ? { offset, pageSize: PAGE_SIZE, total: result.data.total, hrefFor: (next) => `/people?offset=${next}` }
              : undefined
          }
          empty={
            <Empty className="border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Users />
                </EmptyMedia>
                <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
                <EmptyDescription>{t('empty')}</EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <InviteDialog />
              </EmptyContent>
            </Empty>
          }
        />
      )}
    </div>
  );
}

/** Sends a pending invitation again; the previous link stops working. */
function Resend({ account }: { account: Account }) {
  const t = useTranslations('people');
  const errors = useTranslations('errors');
  const invite = useInvite();

  async function resend() {
    const result = await invite.mutateAsync({
      email: account.email,
      locale: account.locale,
      isAdmin: account.isAdmin,
    });
    if (result.ok) toast.success(t('resent', { email: account.email }));
    else toast.error(errors(result.error.code));
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          disabled={invite.isPending}
          aria-label={t('resendFor', { email: account.email })}
          onClick={() => void resend()}
        >
          {invite.isPending ? <Spinner /> : <MailPlus aria-hidden />}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{t('resend')}</TooltipContent>
    </Tooltip>
  );
}

/** Another administrator removes a person's second step when they lost their device. */
function ResetSecondStep({ account }: { account: Account }) {
  const t = useTranslations('people');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const reset = useResetSecondStep();
  // Nobody resets their own second step: they turn it off in Account, with a code.
  const me = useCurrentUser();

  async function confirm() {
    const result = await reset.mutateAsync(account.id);
    if (result.ok) toast.success(t('secondStepReset', { email: account.email }));
    else toast.error(errors(result.error.code));
  }

  if (me.id === account.id) return null;
  return (
    <AlertDialog>
      <Tooltip>
        <TooltipTrigger asChild>
          <AlertDialogTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              disabled={reset.isPending}
              aria-label={t('resetSecondStepFor', { email: account.email })}
            >
              {reset.isPending ? <Spinner /> : <ShieldOff aria-hidden />}
            </Button>
          </AlertDialogTrigger>
        </TooltipTrigger>
        <TooltipContent>{t('resetSecondStep')}</TooltipContent>
      </Tooltip>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t('resetSecondStepTitle', { email: account.email })}</AlertDialogTitle>
          <AlertDialogDescription>{t('resetSecondStepDescription')}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
          <AlertDialogAction onClick={() => void confirm()}>{t('resetSecondStep')}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
