'use client';

import { Archive } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import type { ReactNode } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { cn } from '@/components/shared/utils';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Link } from '@/i18n/navigation';

import { RoleBadge } from './role-badge';
import type { Project } from './schema';
import { useProject } from './useProjects';

type Tab = 'settings' | 'members' | 'audit';

/**
 * Header and tabs of a project page. The project comes from `?id=`; when it
 * cannot be shown (not a member, or gone), the page says so and links back.
 */
export function ProjectFrame({ tab, children }: { tab: Tab; children: (project: Project) => ReactNode }) {
  const t = useTranslations('project');
  const common = useTranslations('common');
  const id = useSearchParams().get('id') ?? '';
  const query = useProject(id);
  const result = query.data;

  if (!id) return <Unavailable />;
  if (!result) return <PageSkeleton />;
  if (!result.ok) {
    if (result.error.code === 'PROJECT_NOT_FOUND') return <Unavailable />;
    return (
      <ErrorAlert
        error={result.error}
        action={
          <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
            {common('retry')}
          </Button>
        }
      />
    );
  }

  const project = result.data;
  return (
    <div className="grid gap-6">
      <header className="grid gap-2">
        <Link href="/" className="w-fit text-sm text-muted-foreground hover:text-foreground">
          ← {t('backToProjects')}
        </Link>
        <div className="flex flex-wrap items-center gap-2">
          <h1 className="text-2xl font-semibold tracking-tight">{project.name}</h1>
          <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
            <span className="sr-only">{t('key')}: </span>
            {project.key}
          </code>
          <RoleBadge role={project.role} />
        </div>
      </header>
      {project.archivedAt ? (
        <Alert>
          <Archive aria-hidden />
          <AlertDescription>{t('archivedNotice')}</AlertDescription>
        </Alert>
      ) : null}
      <nav aria-label={project.name} className="flex gap-1 border-b">
        {tabs(project).map((value) => (
          <Link
            key={value}
            href={`/projects/${value}?id=${project.id}`}
            aria-current={value === tab ? 'page' : undefined}
            className={cn(
              '-mb-px border-b-2 px-3 py-2 text-sm font-medium',
              value === tab
                ? 'border-primary text-foreground'
                : 'border-transparent text-muted-foreground hover:text-foreground',
            )}
          >
            {t(value)}
          </Link>
        ))}
      </nav>
      {children(project)}
    </div>
  );
}

/** The audit log is for owners only. */
function tabs(project: Project): Tab[] {
  return project.role === 'owner' ? ['settings', 'members', 'audit'] : ['settings', 'members'];
}

function Unavailable() {
  const t = useTranslations('project');
  return (
    <div className="grid gap-4">
      <ErrorAlert error={{ code: 'PROJECT_NOT_FOUND' }} title={t('notFoundTitle')} />
      <Button asChild variant="outline" className="w-fit">
        <Link href="/">{t('backToProjects')}</Link>
      </Button>
    </div>
  );
}
