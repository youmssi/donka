export type ProjectSection = 'settings' | 'members' | 'audit';

/** A project page's address: by key, short and readable (`/projects/members?p=credit-pme`). */
export function projectHref(section: ProjectSection, key: string, extra?: Record<string, string>): string {
  const params = new URLSearchParams({ p: key, ...extra });
  return `/projects/${section}?${params}`;
}
