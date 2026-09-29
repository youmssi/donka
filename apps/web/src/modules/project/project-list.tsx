'use client';

import { Archive, FolderOpen } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { PageHeader } from '@/components/shared/layout/page-header';
import { Button } from '@/components/ui/button';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Link, useRouter } from '@/i18n/navigation';
import { useCurrentUser } from '@/modules/identity';

import { CreateProjectDialog } from './create-project-dialog';
import { projectHref } from './links';
import { PAGE_SIZE } from './project.service';
import { RoleBadge } from './role-badge';
import type { ProjectSummary } from './schema';
import { useProjectList } from './useProjects';

type View = 'active' | 'archived';

const viewHref = (view: View, offset = 0) => {
  const params = new URLSearchParams(view === 'archived' ? { view } : {});
  if (offset) params.set('offset', String(offset));
  const query = params.toString();
  return query ? `/?${query}` : '/';
};

/** The projects the signed-in user belongs to (`?view=archived` for archived ones). */
export function ProjectListPage() {
  const t = useTranslations('projects');
  const common = useTranslations('common');
  const user = useCurrentUser();
  const router = useRouter();
  const searchParams = useSearchParams();
  const view: View = searchParams.get('view') === 'archived' ? 'archived' : 'active';
  const offset = Math.max(0, Number(searchParams.get('offset')) || 0);
  const list = useProjectList(view === 'archived', offset);
  const result = list.data;

  const columns: DataTableColumn<ProjectSummary>[] = [
    {
      id: 'project',
      header: t('project'),
      cell: ({ row }) => (
        <div className="grid min-w-0 gap-0.5">
          <Link
            href={projectHref('members', row.original.key)}
            className="w-fit font-medium underline-offset-4 hover:underline"
          >
            {row.original.name}
          </Link>
          {row.original.description ? (
            <span className="line-clamp-1 max-w-xl text-sm text-muted-foreground">{row.original.description}</span>
          ) : null}
        </div>
      ),
      meta: { className: 'whitespace-normal' },
    },
    {
      id: 'key',
      header: t('key'),
      cell: ({ row }) => <code className="font-mono text-xs text-muted-foreground">{row.original.key}</code>,
      meta: { className: 'hidden sm:table-cell' },
    },
    ...(view === 'archived'
      ? [
          {
            id: 'archivedAt',
            header: t('archivedOn'),
            cell: ({ row }) => (row.original.archivedAt ? <When value={row.original.archivedAt} /> : null),
            meta: { className: 'hidden text-muted-foreground md:table-cell' },
          } satisfies DataTableColumn<ProjectSummary>,
        ]
      : []),
    {
      id: 'role',
      header: t('yourRole'),
      cell: ({ row }) => <RoleBadge role={row.original.role} />,
      meta: { className: 'w-32' },
    },
  ];

  return (
    <div className="grid gap-4">
      <PageHeader title={t('title')} actions={user.isAdmin ? <CreateProjectDialog /> : null} />
      <Tabs value={view} onValueChange={(next) => router.replace(viewHref(next as View))}>
        <TabsList>
          <TabsTrigger value="active">{t('active')}</TabsTrigger>
          <TabsTrigger value="archived">{t('archived')}</TabsTrigger>
        </TabsList>
        <TabsContent value={view} className="mt-2">
          {result && !result.ok ? (
            <ErrorAlert
              error={result.error}
              title={t('errorTitle')}
              action={
                <Button variant="outline" size="sm" className="mt-2" onClick={() => void list.refetch()}>
                  {common('retry')}
                </Button>
              }
            />
          ) : (
            <DataTable
              label={t('title')}
              columns={columns}
              data={result?.data.items}
              getRowId={(project) => project.id}
              pagination={
                result
                  ? { offset, pageSize: PAGE_SIZE, total: result.data.total, hrefFor: (next) => viewHref(view, next) }
                  : undefined
              }
              empty={
                <Empty className="border border-dashed">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">{view === 'archived' ? <Archive /> : <FolderOpen />}</EmptyMedia>
                    <EmptyTitle>{view === 'archived' ? t('emptyArchivedTitle') : t('emptyTitle')}</EmptyTitle>
                    <EmptyDescription>
                      {view === 'archived' ? t('emptyArchived') : user.isAdmin ? t('emptyAdmin') : t('emptyMember')}
                    </EmptyDescription>
                  </EmptyHeader>
                  {view === 'active' && user.isAdmin ? (
                    <EmptyContent>
                      <CreateProjectDialog />
                    </EmptyContent>
                  ) : null}
                </Empty>
              }
            />
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
