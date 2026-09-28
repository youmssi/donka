/**
 * Where to go after signing in: only a path inside Studio, never another site
 * (`//evil.example`, `https://…`, `/\evil.example`), so the link cannot be used to phish.
 */
export function safeNext(next: string | null): string {
  if (!next || !next.startsWith('/') || next.startsWith('//') || next.includes('\\')) return '/';
  return next;
}
