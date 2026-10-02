'use client';

import { MoreHorizontal, Package, Rocket, ScrollText } from 'lucide-react';
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
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { Skeleton } from '@/components/ui/skeleton';
import { TestBadge } from '@/modules/decision';
import { projectHref, ProjectFrame, type Project } from '@/modules/project';

import { FrozenDecisions } from './frozen-decisions';
import { NewRelease } from './release-dialog';
import { deployRelease, RELEASES_PAGE_SIZE } from './release.service';
import type { EnvironmentName, ReleaseSummary } from './schema';
import { useRelease, useReleaseList, useReleasesChanged } from './useReleases';

/** Editors and owners release and deploy in an active project. */
export function canRelease(project: Project): boolean {
  return project.role !== 'viewer' && !project.archivedAt;
}

export function ReleasesPage() {
  return (
    <ProjectFrame
      section="releases"
      actions={(project) => (canRelease(project) ? <NewRelease project={project} /> : null)}
    >
      {(project) => <Releases project={project} />}
    </ProjectFrame>
  );
}

/** Where a release is live: one badge per environment. */
export function LiveIn({ environments }: { environments: EnvironmentName[] }) {
  const t = useTranslations('environments');
  return (
    <span className="flex flex-wrap gap-1">
      {environments.map((env) => (
        <Badge key={env} variant={env === 'production' ? 'default' : 'secondary'}>
          {t(env)}
        </Badge>
      ))}
    </span>
  );
}

function Releases({ project }: { project: Project }) {
  const t = useTranslations('releases');
  const common = useTranslations('common');
  const offset = Number(useSearchParams().get('offset')) || 0;
  const query = useReleaseList(project.id, offset);
  const result = query.data;
  const editable = canRelease(project);
  const [viewing, setViewing] = useState<string | null>(null);

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

  const columns: DataTableColumn<ReleaseSummary>[] = [
    {
      id: 'version',
      header: t('version'),
      cell: ({ row }) => (
        <Button
          variant="link"
          className="h-auto p-0 font-mono font-semibold text-foreground"
          onClick={() => setViewing(row.original.id)}
        >
          {row.original.version}
        </Button>
      ),
    },
    {
      id: 'notes',
      header: t('notes'),
      cell: ({ row }) => <span className="text-muted-foreground">{row.original.notes}</span>,
      meta: { className: 'w-full max-w-0 truncate' },
    },
    {
      id: 'live',
      header: t('live'),
      cell: ({ row }) => <LiveIn environments={row.original.liveIn} />,
    },
    {
      id: 'tests',
      header: t('tests'),
      cell: ({ row }) => <TestBadge summary={row.original.tests} />,
      meta: { className: 'hidden md:table-cell' },
    },
    {
      id: 'created',
      header: t('createdAt'),
      cell: ({ row }) => <When value={row.original.createdAt} as="ago" />,
      meta: { className: 'hidden sm:table-cell text-muted-foreground' },
    },
    {
      id: 'by',
      header: t('by'),
      cell: ({ row }) => <Person email={row.original.createdBy.email} />,
      meta: { className: 'hidden lg:table-cell' },
    },
    {
      id: 'actions',
      header: () => <span className="sr-only">{common('actions')}</span>,
      cell: ({ row }) => (
        <ReleaseActions
          project={project}
          release={row.original}
          editable={editable}
          onView={() => setViewing(row.original.id)}
        />
      ),
      meta: { className: 'w-12 text-right' },
    },
  ];

  return (
    <>
      <DataTable
        label={t('title')}
        columns={columns}
        data={result?.data.items}
        getRowId={(release) => release.id}
        pagination={
          result
            ? {
                offset,
                pageSize: RELEASES_PAGE_SIZE,
                total: result.data.total,
                hrefFor: (next) => projectHref('releases', project.key, { offset: String(next) }),
              }
            : undefined
        }
        empty={
          <Empty className="border border-dashed">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Package />
              </EmptyMedia>
              <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
              <EmptyDescription>{editable ? t('empty') : t('emptyReadOnly')}</EmptyDescription>
            </EmptyHeader>
            {editable ? (
              <EmptyContent>
                <NewRelease project={project} />
              </EmptyContent>
            ) : null}
          </Empty>
        }
      />
      <ReleaseSheet project={project} id={viewing} onClose={() => setViewing(null)} />
    </>
  );
}

function ReleaseActions({
  project,
  release,
  editable,
  onView,
}: {
  project: Project;
  release: ReleaseSummary;
  editable: boolean;
  onView: () => void;
}) {
  const t = useTranslations('releases');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [confirming, setConfirming] = useState(false);

  async function onDeploy() {
    const result = await deployRelease(project.id, 'staging', release.id);
    changed();
    if (result.ok) toast.success(t('deploying', { version: release.version }));
    else toast.error(errors(result.error.code));
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={t('actionsFor', { version: release.version })}>
            <MoreHorizontal aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={onView}>
            <ScrollText aria-hidden />
            {t('view')}
          </DropdownMenuItem>
          {editable ? (
            <DropdownMenuItem onSelect={() => setConfirming(true)}>
              <Rocket aria-hidden />
              {t('deployStaging')}
            </DropdownMenuItem>
          ) : null}
        </DropdownMenuContent>
      </DropdownMenu>
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('deployTitle', { version: release.version })}</AlertDialogTitle>
            <AlertDialogDescription>{t('deployConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void onDeploy()}>{t('deployStaging')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

/** A release: its notes and the version of each decision it froze. */
function ReleaseSheet({ project, id, onClose }: { project: Project; id: string | null; onClose: () => void }) {
  const t = useTranslations('releases');
  const common = useTranslations('common');
  // While the sheet slides out, it keeps showing the release it was opened on.
  const [shown, setShown] = useState(id);
  if (id !== null && id !== shown) setShown(id);
  const query = useRelease(project.id, id ?? shown);
  const result = query.data;
  const release = result?.ok ? result.data : null;

  return (
    <Sheet open={id !== null} onOpenChange={(open) => (open ? null : onClose())}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
        <SheetHeader className="border-b">
          <SheetTitle className="font-mono">{release ? release.version : t('release')}</SheetTitle>
          <SheetDescription>
            {release ? (
              <>
                {release.createdBy.email} · <When value={release.createdAt} as="ago" />
              </>
            ) : null}
          </SheetDescription>
        </SheetHeader>
        <div className="grid flex-1 content-start gap-4 overflow-y-auto p-4">
          {result && !result.ok ? (
            <ErrorAlert
              error={result.error}
              action={
                <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
                  {common('retry')}
                </Button>
              }
            />
          ) : !release ? (
            [0, 1, 2].map((index) => <Skeleton key={index} className="h-10 w-full" />)
          ) : (
            <>
              <p className="text-sm break-words whitespace-pre-line">{release.notes}</p>
              <LiveIn environments={release.liveIn} />
              <FrozenDecisions decisions={release.decisions} projectKey={project.key} />
            </>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}
