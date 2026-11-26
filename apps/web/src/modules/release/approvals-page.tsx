'use client';

import { ShieldCheck } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { Person } from '@/components/shared/person';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Link } from '@/i18n/navigation';
import { approvalHref, projectHref, ProjectFrame, type Project } from '@/modules/project';

import { APPROVALS_PAGE_SIZE } from './release.service';
import type { Approval, ApprovalStatus } from './schema';
import { useApprovals } from './useReleases';

export function ApprovalsPage() {
  return <ProjectFrame section="approvals">{(project) => <Approvals project={project} />}</ProjectFrame>;
}

/** A request's status, coloured by what it means for production. */
export function ApprovalBadge({ status }: { status: ApprovalStatus }) {
  const t = useTranslations('approvals');
  const variant = status === 'approved' ? 'default' : status === 'rejected' ? 'destructive' : 'secondary';
  return (
    <Badge
      variant={variant}
      className={status === 'pending' ? 'border-amber-500/50 bg-amber-500/15 text-foreground' : ''}
    >
      {t(status)}
    </Badge>
  );
}

function Approvals({ project }: { project: Project }) {
  const t = useTranslations('approvals');
  const common = useTranslations('common');
  const offset = Number(useSearchParams().get('offset')) || 0;
  const query = useApprovals(project.id, offset);
  const result = query.data;

  if (result && !result.ok) {
    return (
      <ErrorAlert
        error={result.error}
        title={t('errorTitle')}
        action={
          <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
            {common('retry')}
          </Button>
        }
      />
    );
  }

  const columns: DataTableColumn<Approval>[] = [
    {
      id: 'version',
      header: t('release'),
      cell: ({ row }) => (
        <Link
          href={approvalHref(project.key, row.original.id)}
          className="font-mono font-semibold underline-offset-4 hover:underline"
        >
          {row.original.releaseVersion}
        </Link>
      ),
    },
    {
      id: 'status',
      header: t('status'),
      cell: ({ row }) => <ApprovalBadge status={row.original.status} />,
    },
    {
      id: 'notes',
      header: t('notes'),
      cell: ({ row }) => <span className="text-muted-foreground">{row.original.releaseNotes}</span>,
      meta: { className: 'hidden w-full max-w-0 truncate md:table-cell' },
    },
    {
      id: 'requested',
      header: t('requestedBy'),
      cell: ({ row }) => <Person email={row.original.requestedBy.email} />,
      meta: { className: 'hidden lg:table-cell' },
    },
    {
      id: 'when',
      header: t('requestedAt'),
      cell: ({ row }) => <When value={row.original.requestedAt} as="ago" />,
      meta: { className: 'hidden sm:table-cell text-muted-foreground' },
    },
    {
      id: 'decided',
      header: t('decidedBy'),
      cell: ({ row }) => (row.original.decidedBy ? <Person email={row.original.decidedBy.email} /> : null),
      meta: { className: 'hidden lg:table-cell' },
    },
  ];

  return (
    <DataTable
      label={t('title')}
      columns={columns}
      data={result?.data.items}
      getRowId={(approval) => approval.id}
      pagination={
        result
          ? {
              offset,
              pageSize: APPROVALS_PAGE_SIZE,
              total: result.data.total,
              hrefFor: (next) => projectHref('approvals', project.key, { offset: String(next) }),
            }
          : undefined
      }
      empty={
        <Empty className="border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ShieldCheck />
            </EmptyMedia>
            <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
            <EmptyDescription>{t('empty')}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      }
    />
  );
}
