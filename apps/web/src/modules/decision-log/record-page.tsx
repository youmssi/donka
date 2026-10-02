'use client';

import { ChevronRight, CircleCheck, CircleX, Eye, RotateCcw } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import type { ReactNode } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { PageHeader } from '@/components/shared/layout/page-header';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Spinner } from '@/components/ui/spinner';
import { Link } from '@/i18n/navigation';
import { ProjectFrame, projectHref, type Project } from '@/modules/project';

import { Duration, EnvironmentBadge, Outcome } from './decision-log-page';
import type { DecisionRecord, Replay } from './schema';
import { useRecord, useReplay } from './useDecisionLog';

/** One decision a Runtime made: what it was asked, what it answered, and a replay. */
export function RecordPage() {
  const id = useSearchParams().get('r') ?? '';
  return (
    <ProjectFrame section="decision-record" header={() => null}>
      {(project) => <RecordView project={project} id={id} />}
    </ProjectFrame>
  );
}

function RecordView({ project, id }: { project: Project; id: string }) {
  const t = useTranslations('decisionLog');
  const common = useTranslations('common');
  const query = useRecord(project.id, id);
  const replay = useReplay(project.id, id);
  const result = query.data;

  if (!id || (result && !result.ok && result.error.code === 'RECORD_NOT_FOUND')) {
    return (
      <div className="grid gap-4">
        <ErrorAlert error={{ code: 'RECORD_NOT_FOUND' }} title={t('notFoundTitle')} />
        <Button asChild variant="outline" className="w-fit">
          <Link href={projectHref('decision-log', project.key)}>{t('backToLog')}</Link>
        </Button>
      </div>
    );
  }
  if (!result) return <PageSkeleton />;
  if (!result.ok) {
    return (
      <ErrorAlert
        error={result.error}
        action={
          <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
            {common('retry')}
          </Button>
        }
      />
    );
  }

  const record = result.data;
  const replayed = replay.data;
  return (
    <div className="grid gap-6">
      <PageHeader
        title={record.reference ?? t('recordTitle')}
        description={t('recordOf', { key: record.decisionKey })}
        badges={<Outcome record={record} />}
        actions={
          <Button variant="outline" disabled={replay.isPending} onClick={() => replay.mutate()}>
            {replay.isPending ? <Spinner /> : <RotateCcw aria-hidden />}
            {replay.isPending ? t('replaying') : t('replay')}
          </Button>
        }
      />
      <Alert>
        <Eye aria-hidden />
        <AlertDescription>{t('viewAudited')}</AlertDescription>
      </Alert>
      {replayed ? (
        replayed.ok ? (
          <ReplayResult record={record} replay={replayed.data} />
        ) : (
          <ErrorAlert error={replayed.error} title={t('replayFailedTitle')} />
        )
      ) : null}
      <div className="grid items-start gap-6 lg:grid-cols-3">
        <Summary record={record} />
        <div className="grid gap-6 lg:col-span-2">
          <JsonCard title={t('input')} description={t('inputDescription')} value={record.input} />
          {record.status === 'succeeded' ? (
            <JsonCard title={t('output')} description={t('outputDescription')} value={record.output} />
          ) : (
            <JsonCard title={t('error')} description={t('errorDescription')} value={record.error} />
          )}
          {record.trace ? <TraceCard trace={record.trace} /> : null}
        </div>
      </div>
    </div>
  );
}

function Summary({ record }: { record: DecisionRecord }) {
  const t = useTranslations('decisionLog');
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('summary')}</CardTitle>
      </CardHeader>
      <CardContent>
        <dl className="grid gap-3 text-sm">
          <Detail label={t('decision')}>
            <span className="font-mono">{record.decisionKey}</span>
          </Detail>
          <Detail label={t('reference')}>
            {record.reference ?? <span className="text-muted-foreground">{t('noReference')}</span>}
          </Detail>
          <Detail label={t('status')}>{record.status === 'succeeded' ? t('succeeded') : t('failed')}</Detail>
          <Detail label={t('environment')}>
            <EnvironmentBadge environment={record.environment} />
          </Detail>
          <Detail label={t('release')}>
            <span className="font-mono">{record.releaseVersion}</span>
          </Detail>
          <Detail label={t('when')}>
            <When value={record.evaluatedAt} as="dateTime" />
          </Detail>
          <Detail label={t('duration')}>
            <Duration us={record.durationUs} />
          </Detail>
          <Detail label={t('received')}>
            <When value={record.receivedAt} as="dateTime" />
          </Detail>
          <Detail label={t('recordId')}>
            <span className="font-mono text-xs break-all">{record.id}</span>
          </Detail>
        </dl>
      </CardContent>
    </Card>
  );
}

function Detail({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-0.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

/** Whether the release gives the same answer again; when not, both answers side by side. */
function ReplayResult({ record, replay }: { record: DecisionRecord; replay: Replay }) {
  const t = useTranslations('decisionLog');
  const recorded = record.status === 'succeeded' ? record.output : record.error;
  const now = replay.status === 'succeeded' ? replay.output : replay.error;
  return (
    <div className="grid gap-4">
      <Alert variant={replay.identical ? 'default' : 'destructive'}>
        {replay.identical ? <CircleCheck aria-hidden /> : <CircleX aria-hidden />}
        <AlertTitle>{replay.identical ? t('identicalTitle') : t('differentTitle')}</AlertTitle>
        <AlertDescription>
          {replay.identical
            ? t('identical', { version: record.releaseVersion })
            : t('different', { version: record.releaseVersion })}
        </AlertDescription>
      </Alert>
      {replay.identical ? null : (
        <div className="grid gap-6 md:grid-cols-2">
          <JsonCard title={t('recordedAnswer')} value={recorded} />
          <JsonCard title={t('replayedAnswer')} value={now} />
        </div>
      )}
    </div>
  );
}

function JsonCard({ title, description, value }: { title: string; description?: string; value: unknown }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        {description ? <CardDescription>{description}</CardDescription> : null}
      </CardHeader>
      <CardContent>
        <Json value={value} />
      </CardContent>
    </Card>
  );
}

/** The engine's per-node trace, folded away until asked for. */
function TraceCard({ trace }: { trace: unknown }) {
  const t = useTranslations('decisionLog');
  return (
    <Card>
      <details className="group">
        <summary className="cursor-pointer list-none">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ChevronRight className="size-4 transition-transform group-open:rotate-90" aria-hidden />
              {t('trace')}
            </CardTitle>
            <CardDescription>{t('traceDescription')}</CardDescription>
          </CardHeader>
        </summary>
        <CardContent className="pt-4">
          <Json value={trace} />
        </CardContent>
      </details>
    </Card>
  );
}

function Json({ value }: { value: unknown }) {
  return (
    <pre className="max-h-96 overflow-auto rounded-md bg-muted p-3 font-mono text-xs leading-relaxed">
      {JSON.stringify(value ?? null, null, 2)}
    </pre>
  );
}
