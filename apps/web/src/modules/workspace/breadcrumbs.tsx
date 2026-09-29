'use client';

import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { Fragment } from 'react';

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';
import { Skeleton } from '@/components/ui/skeleton';
import { Link, usePathname } from '@/i18n/navigation';
import { projectHome, useOpenProject } from '@/modules/project';

import { placeOf } from './sections';

interface Crumb {
  label: string;
  href?: string;
}

/** Where the person is, from the address: Projects › Crédit PME › Members. */
export function Breadcrumbs() {
  const t = useTranslations('nav');
  const sections = useTranslations('project');
  const place = placeOf(usePathname());
  const open = useOpenProject();
  const decisionKey = useSearchParams().get('d') ?? '';
  const project = place.kind === 'project' && open.query.data?.ok ? open.query.data.data : null;

  let crumbs: Crumb[] | null;
  switch (place.kind) {
    case 'projects':
      crumbs = [{ label: t('projects') }];
      break;
    case 'people':
      crumbs = [{ label: t('people') }];
      break;
    case 'project':
      crumbs = project
        ? [
            { label: t('projects'), href: '/' },
            { label: project.name, href: projectHome(project.key) },
            ...(place.section === 'decision'
              ? [{ label: sections('decisions'), href: projectHome(project.key) }, { label: decisionKey }]
              : [{ label: sections(place.section) }]),
          ]
        : null;
      break;
    default:
      crumbs = [];
  }

  if (!crumbs) return <Skeleton className="h-4 w-48" />;
  return (
    <Breadcrumb className="min-w-0">
      <BreadcrumbList className="flex-nowrap">
        {crumbs.map((crumb, index) => {
          const last = index === crumbs.length - 1;
          return (
            <Fragment key={index}>
              {/* On phones only the page itself is shown; the sidebar has the rest. */}
              <BreadcrumbItem className={last ? 'min-w-0' : 'hidden md:inline-flex'}>
                {last || !crumb.href ? (
                  <BreadcrumbPage className="truncate">{crumb.label}</BreadcrumbPage>
                ) : (
                  <BreadcrumbLink asChild>
                    <Link href={crumb.href}>{crumb.label}</Link>
                  </BreadcrumbLink>
                )}
              </BreadcrumbItem>
              {last ? null : <BreadcrumbSeparator className="hidden md:block" />}
            </Fragment>
          );
        })}
      </BreadcrumbList>
    </Breadcrumb>
  );
}
