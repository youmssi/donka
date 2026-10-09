'use client';

import { ScrollText, Search } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { useAppForm } from '@/components/shared/form';
import { DateRangeFilter } from '@/components/shared/date-range';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Link, useRouter } from '@/i18n/navigation';
import { ProjectFrame, projectHref, recordHref, type Project } from '@/modules/project';

import { PAGE_SIZE } from './decision-log.service';
import {
  duration,
  isEmptyRange,
  isFiltered,
  readFilters,
  toQuery,
  type EnvironmentName,
  type LogFilters,
  type RecordStatus,
  type RecordSummary,
} from './schema';
import { useRecords } from './useDecisionLog';

const ALL = 'all';

/** Every decision the project's Runtimes made, newest first, searchable (any member). */
export function DecisionLogPage() {
  return <ProjectFrame section="decision-log">{(project) => <DecisionLog project={project} />}</ProjectFrame>;
}

function DecisionLog({ project }: { project: Project }) {
  const t = useTranslations('decisionLog');
  const common = useTranslations('common');
  const searchParams = useSearchParams();
  const router = useRouter();
  const filters = readFilters(searchParams);
  const offset = Math.max(0, Number(searchParams.get('offset')) || 0);
  const emptyRange = isEmptyRange(filters);
  const records = useRecords(project.id, toQuery(filters), offset, !emptyRange);
  const result = records.data;
  const filtered = isFiltered(filters);

  const href = (next: LogFilters, nextOffset = 0) => {
    const extra: Record<string, string> = {};
    for (const [name, value] of Object.entries(next)) if (value) extra[name] = value;
    if (nextOffset) extra.offset = String(nextOffset);
    return projectHref('decision-log', project.key, extra);
  };
  const apply = (change: Partial<LogFilters>) => router.replace(href({ ...filters, ...change }));

  const columns: DataTableColumn<RecordSummary>[] = [
    {
      id: 'when',
      header: t('when'),
      cell: ({ row }) => <When value={row.original.evaluatedAt} as="dateTime" />,
      meta: { className: 'w-44 text-muted-foreground' },
    },
    {
      id: 'decision',
      header: t('decision'),
      cell: ({ row }) => (
        <Link href={recordHref(project.key, row.original.id)} className="grid gap-0.5 hover:underline">
          <span className="font-mono text-sm">{row.original.decisionKey}</span>
          <span className="text-xs text-muted-foreground">{row.original.reference ?? t('noReference')}</span>
        </Link>
      ),
    },
    {
      id: 'outcome',
      header: t('outcome'),
      cell: ({ row }) => <Outcome record={row.original} />,
    },
    {
      id: 'environment',
      header: t('environment'),
      cell: ({ row }) => <EnvironmentBadge environment={row.original.environment} />,
      meta: { className: 'hidden md:table-cell' },
    },
    {
      id: 'duration',
      header: t('duration'),
      cell: ({ row }) => <Duration us={row.original.durationUs} />,
      meta: { className: 'hidden text-right text-muted-foreground lg:table-cell' },
    },
  ];

  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <TextFilters key={searchParams.toString()} filters={filters} onApply={apply} />
        <EnvironmentFilter value={filters.environment} onChange={(environment) => apply({ environment })} />
        <StatusFilter value={filters.status} onChange={(status) => apply({ status })} />
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
      </div>

      {emptyRange ? (
        <ErrorAlert error={{ code: 'INVALID_REQUEST' }} title={t('emptyRange')} />
      ) : result && !result.ok ? (
        <ErrorAlert
          error={result.error}
          title={t('errorTitle')}
          action={
            <Button variant="outline" size="sm" className="mt-2" onClick={() => void records.refetch()}>
              {common('retry')}
            </Button>
          }
        />
      ) : (
        <DataTable
          label={t('title')}
          columns={columns}
          data={result?.data.items}
          getRowId={(record) => record.id}
          pagination={
            result
              ? { offset, pageSize: PAGE_SIZE, total: result.data.total, hrefFor: (next) => href(filters, next) }
              : undefined
          }
          empty={
            <Empty className="border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <ScrollText />
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

/** Reference, decision and outcome: typed, applied together with Enter or the search button. */
function TextFilters({ filters, onApply }: { filters: LogFilters; onApply: (change: Partial<LogFilters>) => void }) {
  const t = useTranslations('decisionLog');
  const form = useAppForm({
    defaultValues: {
      reference: filters.reference ?? '',
      decision: filters.decision ?? '',
      outcome: filters.outcome ?? '',
    },
    onSubmit: ({ value }) =>
      onApply({
        reference: value.reference.trim() || undefined,
        decision: value.decision.trim() || undefined,
        outcome: value.outcome.trim() || undefined,
      }),
  });
  const fields = [
    { name: 'reference', label: t('reference'), className: 'w-48' },
    { name: 'decision', label: t('decision'), className: 'w-40' },
    { name: 'outcome', label: t('outcome'), className: 'w-32' },
  ] as const;

  return (
    <form.AppForm>
      <form.Form role="search" className="flex flex-wrap items-center gap-2">
        {fields.map(({ name, label, className }) => (
          <form.AppField key={name} name={name}>
            {(field) => (
              <Input
                aria-label={label}
                placeholder={label}
                className={`h-8 ${className}`}
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </form.AppField>
        ))}
        <Button type="submit" size="sm" variant="secondary">
          <Search aria-hidden />
          {t('search')}
        </Button>
      </form.Form>
    </form.AppForm>
  );
}

function EnvironmentFilter({
  value,
  onChange,
}: {
  value: EnvironmentName | undefined;
  onChange: (environment: EnvironmentName | undefined) => void;
}) {
  const t = useTranslations('decisionLog');
  const environments = useTranslations('environments');
  return (
    <Select
      value={value ?? ALL}
      onValueChange={(next) => onChange(next === ALL ? undefined : (next as EnvironmentName))}
    >
      <SelectTrigger size="sm" className="w-fit min-w-40" aria-label={t('environment')}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL}>{t('allEnvironments')}</SelectItem>
        <SelectItem value="production">{environments('production')}</SelectItem>
        <SelectItem value="staging">{environments('staging')}</SelectItem>
      </SelectContent>
    </Select>
  );
}

function StatusFilter({
  value,
  onChange,
}: {
  value: RecordStatus | undefined;
  onChange: (status: RecordStatus | undefined) => void;
}) {
  const t = useTranslations('decisionLog');
  return (
    <Select value={value ?? ALL} onValueChange={(next) => onChange(next === ALL ? undefined : (next as RecordStatus))}>
      <SelectTrigger size="sm" className="w-fit min-w-40" aria-label={t('status')}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL}>{t('allStatuses')}</SelectItem>
        <SelectItem value="succeeded">{t('succeeded')}</SelectItem>
        <SelectItem value="failed">{t('failed')}</SelectItem>
      </SelectContent>
    </Select>
  );
}

/** The record's outcome; failed evaluations stand out. */
export function Outcome({ record }: { record: Pick<RecordSummary, 'status' | 'outcome'> }) {
  const t = useTranslations('decisionLog');
  if (record.status === 'failed') return <Badge variant="destructive">{t('failed')}</Badge>;
  return record.outcome ? (
    <Badge variant="secondary" className="max-w-48 truncate font-mono">
      {record.outcome}
    </Badge>
  ) : (
    <span className="text-muted-foreground">—</span>
  );
}

export function EnvironmentBadge({ environment }: { environment: EnvironmentName }) {
  const environments = useTranslations('environments');
  return <Badge variant={environment === 'production' ? 'default' : 'outline'}>{environments(environment)}</Badge>;
}

export function Duration({ us }: { us: number }) {
  return <span className="tabular-nums">{duration(us, useLocale())}</span>;
}
