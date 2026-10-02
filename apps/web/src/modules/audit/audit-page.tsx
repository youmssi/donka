'use client';

import { Check, ChevronsUpDown, Download, History, ShieldAlert } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { DateRangeFilter } from '@/components/shared/date-range';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { Person } from '@/components/shared/person';
import { cn } from '@/components/shared/utils';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Link, useRouter } from '@/i18n/navigation';
import { ProjectFrame, projectHref, useMembers, type Project, type Role } from '@/modules/project';
import type { EnvironmentName } from '@/modules/release';

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
  const t = useTranslations('audit');
  const searchParams = useSearchParams();
  return (
    <ProjectFrame
      section="audit"
      actions={(project) =>
        project.role === 'owner' ? (
          <Button asChild variant="outline">
            <a href={exportHref(project.id, toQuery(readFilters(searchParams)))} download>
              <Download aria-hidden />
              {t('export')}
            </a>
          </Button>
        ) : null
      }
    >
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
  const log = useAuditLog(project.id, toQuery(filters), offset, !emptyRange);
  const result = log.data;
  const filtered = Boolean(filters.actor || filters.action || filters.from || filters.to);

  const href = (next: AuditFilters, nextOffset = 0) => {
    const extra: Record<string, string> = {};
    for (const [name, value] of Object.entries(next)) if (value) extra[name] = value;
    if (nextOffset) extra.offset = String(nextOffset);
    return projectHref('audit', project.key, extra);
  };
  const apply = (change: Partial<AuditFilters>) => router.replace(href({ ...filters, ...change }));

  const columns: DataTableColumn<AuditEvent>[] = [
    {
      id: 'when',
      header: t('when'),
      cell: ({ row }) => <When value={row.original.occurredAt} as="ago" />,
      meta: { className: 'w-32 text-muted-foreground' },
    },
    {
      id: 'what',
      header: t('what'),
      cell: ({ row }) => <Change event={row.original} />,
      meta: { className: 'whitespace-normal' },
    },
    {
      id: 'who',
      header: t('who'),
      cell: ({ row }) => <Actor event={row.original} />,
      meta: { className: 'hidden w-64 md:table-cell' },
    },
  ];

  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <PersonFilter project={project} value={filters.actor} onChange={(actor) => apply({ actor })} />
        <ActionFilter value={filters.action} onChange={(action) => apply({ action })} />
        <DateRangeFilter
          from={filters.from}
          to={filters.to}
          label={t('dates')}
          anyLabel={t('anyDate')}
          onChange={(from, to) => apply({ from, to })}
        />
        {filtered ? (
          <Button asChild variant="ghost" size="sm">
            <Link href={href({})}>{t('clearFilters')}</Link>
          </Button>
        ) : null}
        <p className="ml-auto text-xs text-muted-foreground">{t('timeZone', { zone: timeZone() })}</p>
      </div>

      {emptyRange ? (
        <ErrorAlert error={{ code: 'INVALID_REQUEST' }} title={t('emptyRange')} />
      ) : result && !result.ok ? (
        <ErrorAlert
          error={result.error}
          title={t('errorTitle')}
          action={
            <Button variant="outline" size="sm" className="mt-2" onClick={() => void log.refetch()}>
              {common('retry')}
            </Button>
          }
        />
      ) : (
        <DataTable
          label={t('title')}
          columns={columns}
          data={result?.data.items}
          getRowId={(event) => String(event.id)}
          pagination={
            result
              ? { offset, pageSize: PAGE_SIZE, total: result.data.total, hrefFor: (next) => href(filters, next) }
              : undefined
          }
          empty={
            <Empty className="border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <History />
                </EmptyMedia>
                <EmptyTitle>{filtered ? t('emptyFilteredTitle') : t('emptyTitle')}</EmptyTitle>
                <EmptyDescription>{filtered ? t('emptyFiltered') : t('empty')}</EmptyDescription>
              </EmptyHeader>
              {filtered ? (
                <EmptyContent>
                  <Button asChild variant="outline" size="sm">
                    <Link href={href({})}>{t('clearFilters')}</Link>
                  </Button>
                </EmptyContent>
              ) : null}
            </Empty>
          }
        />
      )}
    </div>
  );
}

/** Who made the change; on phones the column is hidden and `Change` says it. */
function Actor({ event }: { event: AuditEvent }) {
  const t = useTranslations('audit');
  return event.actor ? (
    <Person email={event.actor.email} />
  ) : (
    <span className="text-muted-foreground">{t('studio')}</span>
  );
}

function Change({ event }: { event: AuditEvent }) {
  const t = useTranslations('audit');
  const sentence = useSentence(event);
  return (
    <div className="grid gap-0.5">
      <span>{sentence}</span>
      <span className="text-xs text-muted-foreground md:hidden">
        {event.actor ? t('by', { email: event.actor.email }) : t('byStudio')}
      </span>
    </div>
  );
}

/** Pick a person among the project's members, searchable (Popover + Command). */
function PersonFilter({
  project,
  value,
  onChange,
}: {
  project: Project;
  value: string | undefined;
  onChange: (actor: string | undefined) => void;
}) {
  const t = useTranslations('audit');
  const [open, setOpen] = useState(false);
  const members = useMembers(project.id).data;
  const people = members?.ok ? members.data : [];
  const selected = people.find((member) => member.userId === value);
  // Someone who has left the project can still be the filter (from an older link).
  const label = value ? (selected?.email ?? t('formerMember')) : t('everyone');

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          role="combobox"
          aria-expanded={open}
          aria-label={t('person')}
          className="w-56 justify-between font-normal"
        >
          <span className="truncate">{label}</span>
          <ChevronsUpDown className="opacity-50" aria-hidden />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-64 p-0" align="start">
        <Command>
          <CommandInput placeholder={t('searchPerson')} />
          <CommandList>
            <CommandEmpty>{t('noPerson')}</CommandEmpty>
            <CommandGroup>
              {[{ userId: ALL, email: t('everyone') }, ...people].map((member) => {
                const current = (value ?? ALL) === member.userId;
                return (
                  <CommandItem
                    key={member.userId}
                    value={member.email}
                    onSelect={() => {
                      onChange(member.userId === ALL ? undefined : member.userId);
                      setOpen(false);
                    }}
                  >
                    <Check className={cn(current ? 'opacity-100' : 'opacity-0')} aria-hidden />
                    <span className="truncate">{member.email}</span>
                  </CommandItem>
                );
              })}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}

function ActionFilter({
  value,
  onChange,
}: {
  value: AuditAction | undefined;
  onChange: (action: AuditAction | undefined) => void;
}) {
  const t = useTranslations('audit');
  const actions = useTranslations('auditActions');
  return (
    <Select value={value ?? ALL} onValueChange={(next) => onChange(next === ALL ? undefined : (next as AuditAction))}>
      <SelectTrigger size="sm" className="w-56" aria-label={t('action')}>
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
  );
}

/** What happened, in words: the action with the people and values it involved. */
function useSentence(event: AuditEvent): string {
  const t = useTranslations('auditSentences');
  const actions = useTranslations('auditActions');
  const roles = useTranslations('roles');
  const environments = useTranslations('environments');
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
    case 'decision.created':
      return t('decisionCreated', { key: detail(event, 'key') });
    case 'decision.deleted':
      return t('decisionDeleted', { key: detail(event, 'key') });
    case 'decision.version_saved':
      return t('decisionVersionSaved', {
        key: detail(event, 'key'),
        version: detail(event, 'version'),
        message: detail(event, 'message'),
      });
    case 'decision.version_restored':
      return t('decisionVersionRestored', {
        key: detail(event, 'key'),
        version: detail(event, 'version'),
        from: detail(event, 'from'),
      });
    case 'scenario.created':
      return t('scenarioCreated', { name: detail(event, 'name'), key: detail(event, 'key') });
    case 'scenario.updated':
      return t('scenarioUpdated', { name: detail(event, 'name'), key: detail(event, 'key') });
    case 'scenario.deleted':
      return t('scenarioDeleted', { name: detail(event, 'name'), key: detail(event, 'key') });
    case 'release.created':
      return t('releaseCreated', { version: detail(event, 'version') });
    case 'release.deployed':
      return t('releaseDeployed', {
        version: detail(event, 'version'),
        environment: environments(detail(event, 'environment') as EnvironmentName),
      });
    case 'token.issued':
      return t('tokenIssued', {
        name: detail(event, 'name'),
        environment: environments(detail(event, 'environment') as EnvironmentName),
      });
    case 'token.revoked':
      return t('tokenRevoked', {
        name: detail(event, 'name'),
        environment: environments(detail(event, 'environment') as EnvironmentName),
      });
    case 'release.rolled_back':
      return t('releaseRolledBack', {
        from: detail(event, 'from'),
        version: detail(event, 'version'),
        reason: detail(event, 'reason'),
      });
    case 'approval.requested':
      return t('approvalRequested', { version: detail(event, 'version') });
    case 'approval.approved':
      return t('approvalApproved', { version: detail(event, 'version') });
    case 'approval.rejected':
      return t('approvalRejected', { version: detail(event, 'version'), reason: detail(event, 'reason') });
    case 'approval.withdrawn':
      return t('approvalWithdrawn', { version: detail(event, 'version') });
    case 'member.removed':
      return t('memberRemoved', { person, role: role(detail(event, 'role')) });
    case 'decision_record.viewed':
      return t('decisionRecordViewed', { key: detail(event, 'decisionKey'), reference: detail(event, 'reference') });
    case 'decision_record.replayed':
      return t('decisionRecordReplayed', { key: detail(event, 'decisionKey'), reference: detail(event, 'reference') });
    case 'decision_log.purged':
      return t('decisionLogPurged', { records: detail(event, 'records'), days: detail(event, 'retentionDays') });
    case 'decision_log.settings_updated': {
      const field = detail(event, 'to', 'outcomeField');
      return field ? t('decisionLogSettingsUpdated', { field }) : t('decisionLogSettingsCleared');
    }
    default:
      return actions(event.action);
  }
}

/** The viewer's time zone, as the times on this page are shown in it. */
function timeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}
