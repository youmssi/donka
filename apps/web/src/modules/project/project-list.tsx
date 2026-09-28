'use client';

import { Archive, FolderOpen } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';

import { ErrorAlert } from '@/components/shared/error-alert';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { cn } from '@/components/shared/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Link } from '@/i18n/navigation';
import { useCurrentUser } from '@/modules/identity';

import { CreateProjectDialog } from './create-project-dialog';
import { PAGE_SIZE } from './project.service';
import { RoleBadge } from './role-badge';
import { useProjectList } from './useProjects';

/** The projects the signed-in user belongs to (`?view=archived` for archived ones). */
export function ProjectListPage() {
  const t = useTranslations('projects');
  const common = useTranslations('common');
  const user = useCurrentUser();
  const searchParams = useSearchParams();
  const archived = searchParams.get('view') === 'archived';
  const offset = Math.max(0, Number(searchParams.get('offset')) || 0);
  const list = useProjectList(archived, offset);
  const result = list.data;

  const view = (value: 'active' | 'archived') => (value === 'archived' ? '/?view=archived' : '/');

  return (
    <div className="grid gap-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">{t('title')}</h1>
        {user.isAdmin ? <CreateProjectDialog /> : null}
      </div>

      <nav aria-label={t('title')} className="flex gap-1 border-b">
        {(['active', 'archived'] as const).map((value) => {
          const current = (value === 'archived') === archived;
          return (
            <Link
              key={value}
              href={view(value)}
              aria-current={current ? 'page' : undefined}
              className={cn(
                '-mb-px border-b-2 px-3 py-2 text-sm font-medium',
                current
                  ? 'border-primary text-foreground'
                  : 'border-transparent text-muted-foreground hover:text-foreground',
              )}
            >
              {t(value)}
            </Link>
          );
        })}
      </nav>

      {!result ? (
        <PageSkeleton />
      ) : !result.ok ? (
        <ErrorAlert
          error={result.error}
          title={t('errorTitle')}
          action={
            <Button variant="outline" size="sm" className="mt-2" onClick={() => void list.refetch()}>
              {common('retry')}
            </Button>
          }
        />
      ) : result.data.items.length === 0 ? (
        <section className="grid place-items-center gap-3 rounded-xl border border-dashed px-6 py-12 text-center">
          {archived ? (
            <Archive className="size-8 text-muted-foreground" aria-hidden />
          ) : (
            <FolderOpen className="size-8 text-muted-foreground" aria-hidden />
          )}
          <h2 className="font-medium">{archived ? t('emptyArchivedTitle') : t('emptyTitle')}</h2>
          <p className="max-w-md text-sm text-muted-foreground">
            {archived ? t('emptyArchived') : user.isAdmin ? t('emptyAdmin') : t('emptyMember')}
          </p>
          {!archived && user.isAdmin ? <CreateProjectDialog /> : null}
        </section>
      ) : (
        <>
          <ul className="grid gap-3">
            {result.data.items.map((project) => (
              <li key={project.id}>
                <Link
                  href={`/projects/settings?id=${project.id}`}
                  className="grid gap-1 rounded-xl border bg-card p-4 transition-colors hover:bg-accent/50 focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
                >
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="font-medium">{project.name}</span>
                    <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                      {project.key}
                    </code>
                    <span className="ml-auto flex items-center gap-2">
                      {project.archivedAt ? <Badge variant="outline">{t('archivedBadge')}</Badge> : null}
                      <span className="sr-only">{t('yourRole')}:</span>
                      <RoleBadge role={project.role} />
                    </span>
                  </span>
                  {project.description ? (
                    <span className="line-clamp-2 text-sm text-muted-foreground">{project.description}</span>
                  ) : null}
                </Link>
              </li>
            ))}
          </ul>
          {result.data.total > PAGE_SIZE ? (
            <Pager archived={archived} offset={offset} total={result.data.total} />
          ) : null}
        </>
      )}
    </div>
  );
}

function Pager({ archived, offset, total }: { archived: boolean; offset: number; total: number }) {
  const common = useTranslations('common');
  const href = (next: number) =>
    `/?${new URLSearchParams({ ...(archived ? { view: 'archived' } : {}), offset: String(next) })}`;
  const from = offset + 1;
  const to = Math.min(offset + PAGE_SIZE, total);
  return (
    <nav className="flex items-center justify-between gap-3 text-sm" aria-label={common('pageOf', { from, to, total })}>
      <span className="text-muted-foreground">{common('pageOf', { from, to, total })}</span>
      <span className="flex gap-2">
        <Button
          asChild
          variant="outline"
          size="sm"
          aria-disabled={offset === 0}
          className={cn(offset === 0 && 'pointer-events-none opacity-50')}
        >
          <Link href={href(Math.max(0, offset - PAGE_SIZE))}>{common('previous')}</Link>
        </Button>
        <Button
          asChild
          variant="outline"
          size="sm"
          aria-disabled={to >= total}
          className={cn(to >= total && 'pointer-events-none opacity-50')}
        >
          <Link href={href(offset + PAGE_SIZE)}>{common('next')}</Link>
        </Button>
      </span>
    </nav>
  );
}
