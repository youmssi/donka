'use client';

import { FlaskConical, MoreHorizontal, Pencil, Plus, Trash2 } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
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
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Link } from '@/i18n/navigation';
import { decisionHref, projectHref, ProjectFrame, type Project } from '@/modules/project';

import { canEditDecisions } from './decisions-page';
import { deleteScenario, SCENARIOS_PAGE_SIZE } from './decision.service';
import { ScenarioDialog, scenarioValues } from './scenario-dialog';
import type { Scenario } from './schema';
import { useDecisionList, useScenarioList, useScenariosChanged } from './useDecisions';

export function ScenariosPage() {
  return (
    <ProjectFrame section="scenarios" actions={(project) => <NewScenario project={project} />}>
      {(project) => <Scenarios project={project} />}
    </ProjectFrame>
  );
}

/** The project's decisions, for choosing which one a scenario tests. */
function useDecisionChoices(projectId: string) {
  const list = useDecisionList(projectId).data;
  return list?.ok ? list.data.map(({ id, key }) => ({ id, key })) : [];
}

function NewScenario({ project }: { project: Project }) {
  const t = useTranslations('scenarios');
  const decisions = useDecisionChoices(project.id);
  const [open, setOpen] = useState(false);
  if (!canEditDecisions(project) || decisions.length === 0) return null;
  return (
    <>
      <Button onClick={() => setOpen(true)}>
        <Plus aria-hidden />
        {t('create')}
      </Button>
      {open ? (
        <ScenarioDialog
          projectId={project.id}
          decisions={decisions}
          initial={{
            decisionId: decisions[0]?.id ?? '',
            name: '',
            input: '{\n  \n}',
            expected: '{\n  \n}',
            match: 'partial',
          }}
          open
          onOpenChange={setOpen}
        />
      ) : null}
    </>
  );
}

function Scenarios({ project }: { project: Project }) {
  const t = useTranslations('scenarios');
  const common = useTranslations('common');
  const offset = Number(useSearchParams().get('offset')) || 0;
  const query = useScenarioList(project.id, offset);
  const result = query.data;
  const editable = canEditDecisions(project);
  const decisions = useDecisionChoices(project.id);

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

  const columns: DataTableColumn<Scenario>[] = [
    {
      id: 'name',
      header: t('name'),
      cell: ({ row }) => <span className="font-medium">{row.original.name}</span>,
      meta: { className: 'w-full max-w-0 truncate' },
    },
    {
      id: 'decision',
      header: t('decision'),
      cell: ({ row }) => (
        <Link
          href={decisionHref(project.key, row.original.decisionKey)}
          className="font-mono text-sm underline-offset-4 hover:underline"
        >
          {row.original.decisionKey}
        </Link>
      ),
    },
    {
      id: 'match',
      header: t('match'),
      cell: ({ row }) => <Badge variant="secondary">{t(row.original.match)}</Badge>,
      meta: { className: 'hidden sm:table-cell' },
    },
    {
      id: 'updated',
      header: t('changed'),
      cell: ({ row }) => <When value={row.original.updatedAt} as="ago" />,
      meta: { className: 'hidden md:table-cell text-muted-foreground' },
    },
    {
      id: 'by',
      header: t('by'),
      cell: ({ row }) => <Person email={row.original.updatedBy.email} />,
      meta: { className: 'hidden lg:table-cell' },
    },
    ...(editable
      ? [
          {
            id: 'actions',
            header: () => <span className="sr-only">{common('actions')}</span>,
            cell: ({ row }) => <ScenarioActions project={project} scenario={row.original} decisions={decisions} />,
            meta: { className: 'w-12 text-right' },
          } satisfies DataTableColumn<Scenario>,
        ]
      : []),
  ];

  return (
    <DataTable
      label={t('title')}
      columns={columns}
      data={result?.data.items}
      getRowId={(scenario) => scenario.id}
      pagination={
        result
          ? {
              offset,
              pageSize: SCENARIOS_PAGE_SIZE,
              total: result.data.total,
              hrefFor: (next) => projectHref('scenarios', project.key, { offset: String(next) }),
            }
          : undefined
      }
      empty={
        <Empty className="border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FlaskConical />
            </EmptyMedia>
            <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
            <EmptyDescription>
              {!editable ? t('emptyReadOnly') : decisions.length ? t('empty') : t('emptyNoDecision')}
            </EmptyDescription>
          </EmptyHeader>
          {editable ? (
            <EmptyContent>
              <NewScenario project={project} />
            </EmptyContent>
          ) : null}
        </Empty>
      }
    />
  );
}

function ScenarioActions({
  project,
  scenario,
  decisions,
}: {
  project: Project;
  scenario: Scenario;
  decisions: { id: string; key: string }[];
}) {
  const t = useTranslations('scenarios');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useScenariosChanged(project.id);
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);

  async function onDelete() {
    const result = await deleteScenario(project.id, scenario.id);
    if (result.ok) {
      changed();
      toast.success(t('deleted', { name: scenario.name }));
    } else {
      toast.error(errors(result.error.code));
    }
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={t('actionsFor', { name: scenario.name })}>
            <MoreHorizontal aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => setEditing(true)}>
            <Pencil aria-hidden />
            {t('edit')}
          </DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onSelect={() => setConfirming(true)}>
            <Trash2 aria-hidden />
            {t('delete')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      {editing ? (
        <ScenarioDialog
          projectId={project.id}
          decisions={decisions}
          scenarioId={scenario.id}
          initial={scenarioValues(scenario)}
          open
          onOpenChange={setEditing}
        />
      ) : null}
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('deleteConfirmTitle', { name: scenario.name })}</AlertDialogTitle>
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
