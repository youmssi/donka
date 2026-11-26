'use client';

import { Archive } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useEffect, type ReactNode } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { PageHeader } from '@/components/shared/layout/page-header';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Link, useRouter } from '@/i18n/navigation';

import { projectHref, type ProjectSection } from './links';
import type { Project } from './schema';
import { useOpenProject } from './useProjects';

interface ProjectFrameProps {
  section: ProjectSection;
  /** In place of the title line (the decision editor has its own toolbar). */
  header?: (project: Project) => ReactNode;
  /** Buttons next to the page title (add a member, export…). */
  actions?: (project: Project) => ReactNode;
  children: (project: Project) => ReactNode;
}

/**
 * A page of one project. The project comes from `?p=<key>`; when it cannot be
 * shown (not a member, or gone), the page says so and links back. The project's
 * name, your role in it and its sections are in the sidebar and breadcrumb, so
 * the title names the section.
 */
export function ProjectFrame({ section, header, actions, children }: ProjectFrameProps) {
  const t = useTranslations('project');
  const common = useTranslations('common');
  const { requested, legacyId, query } = useOpenProject();
  const result = query.data;
  const params = useSearchParams();
  const router = useRouter();

  // A link made with the project's id becomes the short one, filters kept.
  const key = result?.ok ? result.data.key : null;
  useEffect(() => {
    if (!legacyId || !key) return;
    const rest = Object.fromEntries([...params].filter(([name]) => name !== 'id'));
    router.replace(projectHref(section, key, rest));
  }, [legacyId, key, params, router, section]);

  if (!requested) return <Unavailable />;
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
      {header ? header(project) : <PageHeader title={t(section)} actions={actions?.(project)} />}
      {project.archivedAt ? (
        <Alert>
          <Archive aria-hidden />
          <AlertDescription>{t('archivedNotice')}</AlertDescription>
        </Alert>
      ) : null}
      {children(project)}
    </div>
  );
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
