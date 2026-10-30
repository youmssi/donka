import type { LucideIcon } from 'lucide-react';
import {
  FlaskConical,
  FolderKanban,
  History,
  Package,
  RadioTower,
  ScrollText,
  Server,
  Settings,
  ShieldCheck,
  Users,
  UserSearch,
  UsersRound,
  Workflow,
} from 'lucide-react';

import type { Project, ProjectSection } from '@/modules/project';

/** Where the person is: drives the sidebar highlight, the breadcrumb and the command menu. */
export type Place =
  | { kind: 'projects' }
  | { kind: 'people' }
  | { kind: 'runtimes' }
  | { kind: 'settings' }
  | { kind: 'project'; section: ProjectSection }
  | { kind: 'other' };

export function placeOf(pathname: string): Place {
  if (pathname === '/') return { kind: 'projects' };
  if (pathname.startsWith('/people')) return { kind: 'people' };
  if (pathname.startsWith('/runtimes')) return { kind: 'runtimes' };
  if (pathname.startsWith('/settings')) return { kind: 'settings' };
  const match =
    /^\/projects\/(decisions|decision|scenarios|releases|environments|approvals|approval|decision-log|decision-record|people-requests|settings|members|audit)\/?$/.exec(
      pathname,
    );
  if (match) return { kind: 'project', section: match[1] as ProjectSection };
  return { kind: 'other' };
}

export const PROJECTS_ICON = FolderKanban;
export const PEOPLE_ICON = UsersRound;
export const RUNTIMES_ICON = RadioTower;
export const SETTINGS_ICON = Settings;

/** The section a page belongs to in the navigation: a decision's editor is under Decisions,
 * a request's review under Approvals, a record under the Decision log. */
export function navSection(section: ProjectSection): ProjectSection {
  if (section === 'decision') return 'decisions';
  if (section === 'approval') return 'approvals';
  if (section === 'decision-record') return 'decision-log';
  return section;
}

/** A project's sections a member may open: people's requests and the audit log are for owners. */
export function projectSections(project: Project): { section: ProjectSection; icon: LucideIcon }[] {
  return [
    { section: 'decisions', icon: Workflow },
    { section: 'scenarios', icon: FlaskConical },
    { section: 'releases', icon: Package },
    { section: 'environments', icon: Server },
    { section: 'approvals', icon: ShieldCheck },
    { section: 'decision-log', icon: ScrollText },
    { section: 'members', icon: Users },
    { section: 'settings', icon: Settings },
    ...(project.role === 'owner'
      ? [
          { section: 'people-requests' as const, icon: UserSearch },
          { section: 'audit' as const, icon: History },
        ]
      : []),
  ];
}
