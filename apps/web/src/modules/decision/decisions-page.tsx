'use client';

import { MoreHorizontal, Trash2, Workflow } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
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
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Link } from '@/i18n/navigation';
import { decisionHref, ProjectFrame, type Project } from '@/modules/project';

import { CreateDecisionDialog } from './create-decision-dialog';
import { splitKey, type DecisionSummary } from './schema';
import { useDecisionList, useDeleteDecision } from './useDecisions';

/** Editors and owners change decisions of an active project; viewers open and simulate them. */
export function canEditDecisions(project: Project): boolean {
  return project.role !== 'viewer' && !project.archivedAt;
}

export function DecisionsPage() {
  return (
    <ProjectFrame
      section="decisions"
      actions={(project) => (canEditDecisions(project) ? <CreateDecisionDialog project={project} /> : null)}
    >
      {(project) => <Decisions project={project} />}
    </ProjectFrame>
  );
}

function Decisions({ project }: { project: Project }) {
  const t = useTranslations('decisions');
  const common = useTranslations('common');
  const query = useDecisionList(project.id);
  const result = query.data;
  const editable = canEditDecisions(project);

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

  const columns: DataTableColumn<DecisionSummary>[] = [
    {
      id: 'key',
      header: t('decision'),
      cell: ({ row }) => {
        const { folder, name } = splitKey(row.original.key);
        return (
          <Link
            href={decisionHref(project.key, row.original.key)}
            className="font-mono text-sm underline-offset-4 hover:underline"
          >
            <span className="text-muted-foreground">{folder}</span>
            <span className="font-medium">{name}</span>
          </Link>
        );
      },
      meta: { className: 'w-full max-w-0 truncate' },
    },
    {
      id: 'updated',
      header: t('updated'),
      cell: ({ row }) => <When value={row.original.updatedAt} as="ago" />,
      meta: { className: 'text-muted-foreground' },
    },
    {
      id: 'by',
      header: t('by'),
      cell: ({ row }) => <Person email={row.original.updatedBy.email} />,
      meta: { className: 'hidden md:table-cell' },
    },
    ...(editable
      ? [
          {
            id: 'actions',
            header: () => <span className="sr-only">{common('actions')}</span>,
            cell: ({ row }) => <DecisionActions project={project} decision={row.original} />,
            meta: { className: 'w-12 text-right' },
          } satisfies DataTableColumn<DecisionSummary>,
        ]
      : []),
  ];

  return (
    <DataTable
      label={t('title')}
      columns={columns}
      data={result?.data}
      getRowId={(decision) => decision.id}
      empty={
        <Empty className="border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Workflow />
            </EmptyMedia>
            <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
            <EmptyDescription>{editable ? t('empty') : t('emptyReadOnly')}</EmptyDescription>
          </EmptyHeader>
          {editable ? (
            <EmptyContent>
              <CreateDecisionDialog project={project} />
            </EmptyContent>
          ) : null}
        </Empty>
      }
    />
  );
}

function DecisionActions({ project, decision }: { project: Project; decision: DecisionSummary }) {
  const t = useTranslations('decisions');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const remove = useDeleteDecision(project.id);
  const [confirming, setConfirming] = useState(false);

  async function onDelete() {
    const result = await remove.mutateAsync(decision.id);
    if (result.ok) toast.success(t('deleted', { key: decision.key }));
    else toast.error(errors(result.error.code));
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={t('actionsFor', { key: decision.key })}>
            <MoreHorizontal aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem variant="destructive" onSelect={() => setConfirming(true)}>
            <Trash2 aria-hidden />
            {t('delete')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('deleteConfirmTitle', { key: decision.key })}</AlertDialogTitle>
            <AlertDialogDescription>{t('deleteConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => void onDelete()}>
              {t('delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
