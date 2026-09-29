'use client';

import { MailPlus, Users } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { toast } from 'sonner';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { PageHeader } from '@/components/shared/layout/page-header';
import { Person } from '@/components/shared/person';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Spinner } from '@/components/ui/spinner';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

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
      id: 'since',
      header: t('since'),
      cell: ({ row }) => <When value={row.original.createdAt} />,
      meta: { className: 'hidden text-muted-foreground md:table-cell' },
    },
    {
      id: 'actions',
      header: () => <span className="sr-only">{common('actions')}</span>,
      cell: ({ row }) => (row.original.active ? null : <Resend account={row.original} />),
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
