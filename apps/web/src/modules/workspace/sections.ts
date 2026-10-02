import type { LucideIcon } from 'lucide-react';
import {
  FlaskConical,
  FolderKanban,
  History,
  Package,
  Server,
  Settings,
  ShieldCheck,
  Users,
  UsersRound,
  Workflow,
} from 'lucide-react';

import type { Project, ProjectSection } from '@/modules/project';

/** Where the person is: drives the sidebar highlight, the breadcrumb and the command menu. */
export type Place =
  { kind: 'projects' } | { kind: 'people' } | { kind: 'project'; section: ProjectSection } | { kind: 'other' };

export function placeOf(pathname: string): Place {
  if (pathname === '/') return { kind: 'projects' };
  if (pathname.startsWith('/people')) return { kind: 'people' };
  const match =
    /^\/projects\/(decisions|decision|scenarios|releases|environments|approvals|approval|settings|members|audit)\/?$/.exec(
      pathname,
    );
  if (match) return { kind: 'project', section: match[1] as ProjectSection };
  return { kind: 'other' };
}

export const PROJECTS_ICON = FolderKanban;
export const PEOPLE_ICON = UsersRound;

/** The section a page belongs to in the navigation: a decision's editor is under Decisions,
 * a request's review under Approvals. */
export function navSection(section: ProjectSection): ProjectSection {
  if (section === 'decision') return 'decisions';
  if (section === 'approval') return 'approvals';
  return section;
}

/** A project's sections a member may open: the audit log is for owners. */
export function projectSections(project: Project): { section: ProjectSection; icon: LucideIcon }[] {
  return [
    { section: 'decisions', icon: Workflow },
    { section: 'scenarios', icon: FlaskConical },
    { section: 'releases', icon: Package },
    { section: 'environments', icon: Server },
    { section: 'approvals', icon: ShieldCheck },
    { section: 'members', icon: Users },
    { section: 'settings', icon: Settings },
    ...(project.role === 'owner' ? [{ section: 'audit' as const, icon: History }] : []),
  ];
}
