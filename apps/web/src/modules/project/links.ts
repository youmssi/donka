/** A project's pages; `decision` is the editor of one decision, `approval` the review of one
 * request, `decision-record` one decision the Runtime made. */
export type ProjectSection =
  | 'decisions'
  | 'decision'
  | 'scenarios'
  | 'releases'
  | 'environments'
  | 'approvals'
  | 'approval'
  | 'decision-log'
  | 'decision-record'
  | 'members'
  | 'settings'
  | 'audit';

/** A project page's address: by key, short and readable (`/projects/members?p=credit-pme`). */
export function projectHref(section: ProjectSection, key: string, extra?: Record<string, string>): string {
  const params = new URLSearchParams({ p: key, ...extra });
  return `/projects/${section}?${params}`;
}

/** Where opening a project leads: its decisions. */
export function projectHome(key: string): string {
  return projectHref('decisions', key);
}

/** The editor of one decision (`/projects/decision?p=credit-pme&d=bureau/normalize`). */
export function decisionHref(projectKey: string, decisionKey: string): string {
  return projectHref('decision', projectKey, { d: decisionKey });
}

/** The review of one production request (`/projects/approval?p=credit-pme&a=<id>`). */
export function approvalHref(projectKey: string, id: string): string {
  return projectHref('approval', projectKey, { a: id });
}

/** One decision the Runtime made (`/projects/decision-record?p=credit-pme&r=<id>`). */
export function recordHref(projectKey: string, id: string): string {
  return projectHref('decision-record', projectKey, { r: id });
}
