'use client';

import { Download, History, ShieldAlert } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useId } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Pager } from '@/components/shared/pager';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Link, useRouter } from '@/i18n/navigation';
import { ProjectFrame, useMembers, type Project, type Role } from '@/modules/project';

import { exportHref, PAGE_SIZE } from './audit.service';
import {
  detail,
  isEmptyRange,
  PROJECT_ACTIONS,
  readFilters,
  toQuery,
  type AuditAction,
  type AuditEvent,
  type AuditFilters,
} from './schema';
import { useAuditLog } from './useAudit';

const ALL = 'all';

export function ProjectAuditPage() {
  return (
    <ProjectFrame tab="audit">
      {(project) => (project.role === 'owner' ? <AuditLog project={project} /> : <OwnersOnly />)}
    </ProjectFrame>
  );
}

function OwnersOnly() {
  const t = useTranslations('audit');
  return (
    <Alert>
      <ShieldAlert aria-hidden />
      <AlertDescription>{t('ownersOnly')}</AlertDescription>
    </Alert>
  );
}

function AuditLog({ project }: { project: Project }) {
  const t = useTranslations('audit');
  const common = useTranslations('common');
  const searchParams = useSearchParams();
  const router = useRouter();
  const filters = readFilters(searchParams);
  const offset = Math.max(0, Number(searchParams.get('offset')) || 0);
  const emptyRange = isEmptyRange(filters);
  const query = toQuery(filters);
  const log = useAuditLog(project.id, query, offset, !emptyRange);
  const result = log.data;
  const filtered = Boolean(filters.actor || filters.action || filters.from || filters.to);

  const href = (next: AuditFilters, nextOffset = 0) => {
    const params = new URLSearchParams({ id: project.id });
    for (const [name, value] of Object.entries(next)) if (value) params.set(name, value);
    if (nextOffset) params.set('offset', String(nextOffset));
    return `/projects/audit?${params}`;
  };
  const apply = (change: Partial<AuditFilters>) => router.replace(href({ ...filters, ...change }));

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description', { zone: timeZone() })}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6">
        <Filters project={project} filters={filters} onChange={apply} />
        <div className="flex flex-wrap items-center gap-2">
          <Button asChild variant="outline" size="sm">
            <a href={exportHref(project.id, query)} download>
              <Download aria-hidden />
              {t('export')}
            </a>
          </Button>
          {filtered ? (
            <Button asChild variant="ghost" size="sm">
              <Link href={href({})}>{t('clearFilters')}</Link>
            </Button>
          ) : null}
        </div>

        {emptyRange ? (
          <ErrorAlert error={{ code: 'INVALID_REQUEST' }} title={t('emptyRange')} />
        ) : !result ? (
          <PageSkeleton />
        ) : !result.ok ? (
          <ErrorAlert
            error={result.error}
            title={t('errorTitle')}
            action={
              <Button variant="outline" size="sm" className="mt-2" onClick={() => void log.refetch()}>
                {common('retry')}
              </Button>
            }
          />
        ) : result.data.items.length === 0 ? (
          <section className="grid place-items-center gap-3 rounded-xl border border-dashed px-6 py-12 text-center">
            <History className="size-8 text-muted-foreground" aria-hidden />
            <p className="text-sm text-muted-foreground">{filtered ? t('emptyFiltered') : t('empty')}</p>
          </section>
        ) : (
          <>
            <ol className="divide-y">
              {result.data.items.map((event) => (
                <EventRow key={event.id} event={event} />
              ))}
            </ol>
            {result.data.total > PAGE_SIZE ? (
              <Pager
                hrefFor={(next) => href(filters, next)}
                offset={offset}
                pageSize={PAGE_SIZE}
                total={result.data.total}
              />
            ) : null}
          </>
        )}
      </CardContent>
    </Card>
  );
}

function Filters({
  project,
  filters,
  onChange,
}: {
  project: Project;
  filters: AuditFilters;
  onChange: (change: Partial<AuditFilters>) => void;
}) {
  const t = useTranslations('audit');
  const actions = useTranslations('auditActions');
  const members = useMembers(project.id).data;
  const people = members?.ok ? members.data : [];
  const ids = { person: useId(), action: useId(), from: useId(), to: useId() };
  // Someone who has left the project can still be the filter (from an older link).
  const formerMember = filters.actor && !people.some((member) => member.userId === filters.actor);

  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <div className="grid gap-2">
        <Label htmlFor={ids.person}>{t('person')}</Label>
        <Select
          value={filters.actor ?? ALL}
          onValueChange={(value) => onChange({ actor: value === ALL ? undefined : value })}
        >
          <SelectTrigger id={ids.person} className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>{t('everyone')}</SelectItem>
            {people.map((member) => (
              <SelectItem key={member.userId} value={member.userId}>
                {member.email}
              </SelectItem>
            ))}
            {formerMember ? <SelectItem value={filters.actor ?? ALL}>{t('formerMember')}</SelectItem> : null}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-2">
        <Label htmlFor={ids.action}>{t('action')}</Label>
        <Select
          value={filters.action ?? ALL}
          onValueChange={(value) => onChange({ action: value === ALL ? undefined : (value as AuditAction) })}
        >
          <SelectTrigger id={ids.action} className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>{t('allActions')}</SelectItem>
            {PROJECT_ACTIONS.map((action) => (
              <SelectItem key={action} value={action}>
                {actions(action)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-2">
        <Label htmlFor={ids.from}>{t('from')}</Label>
        <Input
          id={ids.from}
          type="date"
          value={filters.from ?? ''}
          max={filters.to}
          onChange={(event) => onChange({ from: event.target.value || undefined })}
        />
      </div>
      <div className="grid gap-2">
        <Label htmlFor={ids.to}>{t('to')}</Label>
        <Input
          id={ids.to}
          type="date"
          value={filters.to ?? ''}
          min={filters.from}
          onChange={(event) => onChange({ to: event.target.value || undefined })}
        />
      </div>
    </div>
  );
}

function EventRow({ event }: { event: AuditEvent }) {
  const t = useTranslations('audit');
  const locale = useLocale();
  const sentence = useSentence(event);
  const when = new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(
    new Date(event.occurredAt),
  );
  return (
    <li className="grid gap-1 py-3 sm:grid-cols-[12rem_1fr] sm:gap-4">
      <time dateTime={event.occurredAt} className="text-sm text-muted-foreground tabular-nums">
        {when}
      </time>
      <div className="min-w-0">
        <p className="break-words">{sentence}</p>
        <p className="truncate text-sm text-muted-foreground">
          {event.actor ? t('by', { email: event.actor.email }) : t('byStudio')}
        </p>
      </div>
    </li>
  );
}

/** What happened, in words: the action with the people and values it involved. */
function useSentence(event: AuditEvent): string {
  const t = useTranslations('auditSentences');
  const actions = useTranslations('auditActions');
  const roles = useTranslations('roles');
  const person = event.target?.email ?? '';
  const role = (value: string) => (value ? roles(value as Role) : value);
  switch (event.action) {
    case 'project.created':
      return t('projectCreated', { name: detail(event, 'name') });
    case 'project.archived':
      return t('projectArchived');
    case 'project.restored':
      return t('projectRestored');
    case 'project.updated': {
      const from = detail(event, 'from', 'name');
      const to = detail(event, 'to', 'name');
      return from !== to ? t('projectRenamed', { from, to }) : t('projectDescribed');
    }
    case 'member.added':
      return t('memberAdded', { person, role: role(detail(event, 'role')) });
    case 'member.role_changed':
      return t('memberRoleChanged', {
        person,
        from: role(detail(event, 'from')),
        to: role(detail(event, 'to')),
      });
    case 'member.removed':
      return t('memberRemoved', { person, role: role(detail(event, 'role')) });
    default:
      return actions(event.action);
  }
}

/** The viewer's time zone, as the times on this page are shown in it. */
function timeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}
