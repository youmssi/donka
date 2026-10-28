'use client';

import { Download, Eraser, Search, ShieldAlert, UserSearch } from 'lucide-react';
import { useLocale, useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { useDateFormat, When } from '@/components/shared/format';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { FieldGroup } from '@/components/ui/field';
import { Spinner } from '@/components/ui/spinner';
import { Link } from '@/i18n/navigation';
import { ProjectFrame, recordHref, type Project } from '@/modules/project';

import { EnvironmentBadge, Outcome } from './decision-log-page';
import {
  blockingRule,
  dayDate,
  IDENTIFIER_MAX,
  personSchema,
  reportSchema,
  type ExplainLanguage,
  type PersonRecord,
  type ReportValues,
} from './schema';
import { useAnonymisePerson, useFindPerson, useLogSettings, usePersonReport } from './useDecisionLog';

/** An owner answers a person's request: find their decisions, give them a report, anonymise them. */
export function PeopleRequestsPage() {
  return (
    <ProjectFrame section="people-requests">
      {(project) => (project.role === 'owner' ? <PeopleRequests project={project} /> : <OwnersOnly />)}
    </ProjectFrame>
  );
}

function OwnersOnly() {
  const t = useTranslations('peopleRequests');
  return (
    <Alert>
      <ShieldAlert aria-hidden />
      <AlertDescription>{t('ownersOnly')}</AlertDescription>
    </Alert>
  );
}

interface Found {
  identifier: string;
  records: PersonRecord[];
}

function PeopleRequests({ project }: { project: Project }) {
  const t = useTranslations('peopleRequests');
  const find = useFindPerson(project.id);
  const [found, setFound] = useState<Found | null>(null);
  const [error, setError] = useState<ActionError | null>(null);

  async function search(identifier: string) {
    setError(null);
    const result = await find.mutateAsync(identifier);
    if (result.ok) setFound({ identifier, records: result.data });
    else {
      setFound(null);
      setError(result.error);
    }
  }

  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle>{t('findTitle')}</CardTitle>
          <CardDescription>{t('findDescription')}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <SearchForm onSearch={search} />
          {error ? <ErrorAlert error={error} /> : null}
        </CardContent>
      </Card>
      {found ? (
        <Results
          // A new search starts the actions afresh.
          key={`${found.identifier}|${found.records.length}`}
          project={project}
          found={found}
          onChanged={() => void search(found.identifier)}
        />
      ) : null}
    </div>
  );
}

/** The identifier stays in the page: never in its address, never stored. */
function SearchForm({ onSearch }: { onSearch: (identifier: string) => Promise<void> }) {
  const t = useTranslations('peopleRequests');
  const form = useAppForm({
    defaultValues: { identifier: '' },
    validators: { onSubmit: personSchema },
    onSubmit: ({ value }) => onSearch(value.identifier.trim()),
  });
  return (
    <form.AppForm>
      <form.Form role="search" className="flex flex-wrap items-start gap-2">
        <FieldGroup className="max-w-md min-w-60 flex-1">
          <form.AppField name="identifier">
            {(field) => (
              <field.TextField
                label={t('identifier')}
                hint={t('identifierHint')}
                autoComplete="off"
                spellCheck={false}
                messageValues={{ max: IDENTIFIER_MAX }}
              />
            )}
          </form.AppField>
        </FieldGroup>
        <form.SubmitButton pendingLabel={t('searching')} className="sm:mt-6">
          <Search aria-hidden />
          {t('search')}
        </form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}

function Results({ project, found, onChanged }: { project: Project; found: Found; onChanged: () => void }) {
  const t = useTranslations('peopleRequests');
  const log = useTranslations('decisionLog');
  const [dialog, setDialog] = useState<'report' | 'anonymise' | null>(null);
  const remaining = found.records.filter((record) => !record.anonymisedAt).length;

  const columns: DataTableColumn<PersonRecord>[] = [
    {
      id: 'when',
      header: log('when'),
      cell: ({ row }) => <When value={row.original.evaluatedAt} as="dateTime" />,
      meta: { className: 'w-44 text-muted-foreground' },
    },
    {
      id: 'decision',
      header: log('decision'),
      cell: ({ row }) => (
        <Link href={recordHref(project.key, row.original.id)} className="grid gap-0.5 hover:underline">
          <span className="font-mono text-sm">{row.original.decisionKey}</span>
          <span className="text-xs text-muted-foreground">{row.original.reference ?? log('noReference')}</span>
        </Link>
      ),
    },
    {
      id: 'outcome',
      header: log('outcome'),
      cell: ({ row }) => <Outcome record={row.original} />,
    },
    {
      id: 'environment',
      header: log('environment'),
      cell: ({ row }) => <EnvironmentBadge environment={row.original.environment} />,
      meta: { className: 'hidden md:table-cell' },
    },
    {
      id: 'anonymised',
      header: t('personalData'),
      cell: ({ row }) =>
        row.original.anonymisedAt ? (
          <Badge variant="outline">{t('anonymised')}</Badge>
        ) : (
          <span className="text-muted-foreground">{t('held')}</span>
        ),
      meta: { className: 'hidden sm:table-cell' },
    },
  ];

  return (
    <Card>
      <CardHeader className="flex flex-wrap items-start justify-between gap-4">
        <div className="grid gap-1.5">
          <CardTitle>
            {found.records.length === 1 ? t('foundOne') : t('foundMany', { count: found.records.length })}
          </CardTitle>
          <CardDescription>{t('foundDescription')}</CardDescription>
        </div>
        {found.records.length > 0 ? (
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" onClick={() => setDialog('report')}>
              <Download aria-hidden />
              {t('report')}
            </Button>
            {remaining > 0 ? (
              <Button variant="outline" onClick={() => setDialog('anonymise')}>
                <Eraser aria-hidden />
                {t('anonymise')}
              </Button>
            ) : null}
          </div>
        ) : null}
      </CardHeader>
      <CardContent>
        <DataTable
          label={t('resultsLabel')}
          columns={columns}
          data={found.records}
          getRowId={(record) => record.id}
          empty={
            <Empty className="border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <UserSearch />
                </EmptyMedia>
                <EmptyTitle>{t('noneTitle')}</EmptyTitle>
                <EmptyDescription>{t('none')}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          }
        />
      </CardContent>
      {dialog === 'report' ? (
        <ReportDialog project={project} identifier={found.identifier} onClose={() => setDialog(null)} />
      ) : null}
      {dialog === 'anonymise' ? (
        <AnonymiseDialog
          project={project}
          identifier={found.identifier}
          records={remaining}
          onClose={() => setDialog(null)}
          onDone={onChanged}
        />
      ) : null}
    </Card>
  );
}

function ReportDialog({ project, identifier, onClose }: { project: Project; identifier: string; onClose: () => void }) {
  const t = useTranslations('peopleRequests');
  const common = useTranslations('common');
  const locale = useLocale();
  const settings = useLogSettings(project.id).data;
  const explainable = settings?.ok ? settings.data.explainEnabled : false;
  const report = usePersonReport(project.id);
  const [error, setError] = useState<ActionError | null>(null);

  const defaultValues: ReportValues = { language: locale === 'fr' ? 'fr' : 'en', explain: false };
  const form = useAppForm({
    defaultValues,
    validators: { onSubmit: reportSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await report.mutateAsync({
        identifier,
        language: value.language as ExplainLanguage,
        explain: explainable && value.explain,
      });
      if (!result.ok) {
        setError(result.error);
        return;
      }
      const url = URL.createObjectURL(result.data.blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = result.data.name;
      link.click();
      URL.revokeObjectURL(url);
      toast.success(t('reportReady', { file: result.data.name }));
      onClose();
    },
  });

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('reportTitle')}</DialogTitle>
          <DialogDescription>{t('reportDescription')}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-4">
              <form.AppField name="language">
                {(field) => (
                  <field.SelectField
                    label={t('language')}
                    hint={t('languageHint')}
                    options={[
                      { value: 'en', label: t('english') },
                      { value: 'fr', label: t('french') },
                    ]}
                  />
                )}
              </form.AppField>
              {explainable ? (
                <form.AppField name="explain">
                  {(field) => <field.CheckboxField label={t('explain')} hint={t('explainHint')} />}
                </form.AppField>
              ) : null}
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('preparing')}>{t('download')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}

function AnonymiseDialog({
  project,
  identifier,
  records,
  onClose,
  onDone,
}: {
  project: Project;
  identifier: string;
  records: number;
  onClose: () => void;
  onDone: () => void;
}) {
  const t = useTranslations('peopleRequests');
  const common = useTranslations('common');
  const anonymise = useAnonymisePerson(project.id);
  const format = useDateFormat();
  const [error, setError] = useState<ActionError | null>(null);
  const rule = error?.code === 'RETENTION_RULE_RUNS' ? blockingRule(error.details) : null;

  async function run() {
    setError(null);
    const result = await anonymise.mutateAsync(identifier);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    toast.success(result.data === 1 ? t('anonymisedOne') : t('anonymisedMany', { count: result.data }));
    onClose();
    onDone();
  }

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('anonymiseTitle')}</DialogTitle>
          <DialogDescription>
            {records === 1 ? t('anonymiseOne') : t('anonymiseMany', { count: records })}
          </DialogDescription>
        </DialogHeader>
        {rule ? (
          <Alert variant="destructive">
            <ShieldAlert aria-hidden />
            <AlertTitle>{t('ruleRuns', { until: format.date(dayDate(rule.until)) })}</AlertTitle>
            <AlertDescription>
              <p>{rule.description}</p>
              {rule.legalReference ? <p className="text-xs">{rule.legalReference}</p> : null}
            </AlertDescription>
          </Alert>
        ) : error ? (
          <ErrorAlert error={error} />
        ) : (
          <Alert>
            <Eraser aria-hidden />
            <AlertDescription>{t('anonymiseKeeps')}</AlertDescription>
          </Alert>
        )}
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            {common('cancel')}
          </Button>
          {rule ? null : (
            <Button variant="destructive" disabled={anonymise.isPending} onClick={() => void run()}>
              {anonymise.isPending ? <Spinner /> : null}
              {anonymise.isPending ? t('anonymising') : t('anonymiseConfirm')}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
