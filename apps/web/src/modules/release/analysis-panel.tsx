'use client';

import { useEffect, useState } from 'react';
import { useTranslations } from 'next-intl';

import { ErrorAlert } from '@/components/shared/error-alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Field, FieldLabel } from '@/components/ui/field';
import { useModule } from '@/modules/licence';

import { cancelAnalysis, latestAnalysis, startAnalysis, getAnalysis } from './release.service';
import type { AnalysisJob, ApprovalReview } from './schema';
import type { Project } from '@/modules/project';

function dateValue(date: Date) {
  return date.toISOString().slice(0, 10);
}

function initialDates() {
  const until = new Date();
  const from = new Date(until);
  from.setUTCDate(from.getUTCDate() - 30);
  return { from: dateValue(from), until: dateValue(until) };
}

export function AnalysisPanel({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals.analysis');
  const licensed = useModule('analysis');
  const owner = project.role === 'owner';
  const choices = review.changes.filter((change) => change.toVersion !== null && change.toVersion !== undefined);
  const [decisionKey, setDecisionKey] = useState(choices[0]?.key ?? '');
  const [dates, setDates] = useState(initialDates);
  const [job, setJob] = useState<AnalysisJob | null>(null);
  const [error, setError] = useState<import('@/components/shared/api').ActionError | null>(null);
  const [busy, setBusy] = useState(false);
  const active = job?.status === 'queued' || job?.status === 'running';

  useEffect(() => {
    if (!owner || !licensed) return;
    let cancelled = false;
    void latestAnalysis(project.id, review.id).then((result) => {
      if (cancelled) return;
      if (result.ok) setJob(result.data);
      else setError(result.error);
    });
    return () => {
      cancelled = true;
    };
  }, [licensed, owner, project.id, review.id]);

  useEffect(() => {
    if (!owner || !licensed || !active || !job) return;
    const timer = setTimeout(() => {
      void getAnalysis(project.id, job.id).then((result) => {
        if (result.ok) setJob(result.data);
        else setError(result.error);
      });
    }, 750);
    return () => clearTimeout(timer);
  }, [active, job, licensed, owner, project.id]);

  async function run() {
    setBusy(true);
    setError(null);
    const result = await startAnalysis(project.id, review.id, {
      decisionKey,
      from: `${dates.from}T00:00:00.000Z`,
      until: `${dates.until}T23:59:59.999Z`,
    });
    if (result.ok) setJob(result.data);
    else setError(result.error);
    setBusy(false);
  }

  async function cancel() {
    if (!job) return;
    setBusy(true);
    const result = await cancelAnalysis(project.id, job.id);
    if (result.ok) setJob(result.data);
    else setError(result.error);
    setBusy(false);
  }

  if (!owner || !licensed) return null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description')}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <p className="text-sm text-muted-foreground">{t('limitation')}</p>
        {error ? <ErrorAlert error={error} /> : null}
        {review.status === 'pending' && choices.length > 0 ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="analysis-decision">{t('decision')}</FieldLabel>
              <select
                id="analysis-decision"
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
                value={decisionKey}
                onChange={(event) => setDecisionKey(event.target.value)}
                disabled={Boolean(active) || busy}
              >
                {choices.map((choice) => (
                  <option key={choice.key} value={choice.key}>
                    {choice.key}
                  </option>
                ))}
              </select>
            </Field>
            <Field>
              <FieldLabel htmlFor="analysis-from">{t('from')}</FieldLabel>
              <input
                id="analysis-from"
                type="date"
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
                value={dates.from}
                onChange={(event) => setDates((current) => ({ ...current, from: event.target.value }))}
                disabled={Boolean(active) || busy}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="analysis-until">{t('until')}</FieldLabel>
              <input
                id="analysis-until"
                type="date"
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
                value={dates.until}
                onChange={(event) => setDates((current) => ({ ...current, until: event.target.value }))}
                disabled={Boolean(active) || busy}
              />
            </Field>
            <div className="flex flex-wrap items-end gap-2">
              <Button
                onClick={() => void run()}
                disabled={busy || Boolean(active) || !decisionKey || !dates.from || !dates.until}
              >
                {t('run')}
              </Button>
              {active ? (
                <Button variant="outline" onClick={() => void cancel()} disabled={busy}>
                  {t('cancel')}
                </Button>
              ) : null}
            </div>
          </div>
        ) : null}
        {job ? <JobResult job={job} /> : null}
      </CardContent>
    </Card>
  );
}

function JobResult({ job }: { job: AnalysisJob }) {
  const t = useTranslations('approvals.analysis');
  const result = job.result;
  return (
    <section aria-label={t('result')} className="grid min-w-0 gap-3 border-t pt-4">
      <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
        <p>{t(`status.${job.status}` as 'status.queued')}</p>
        <p>{t('recordsRead', { count: job.recordsRead })}</p>
      </div>
      {!result ? null : (
        <>
          <p className="text-sm">{t('recordsCompared', { count: result.recordsCompared })}</p>
          {result.truncated ? (
            <p role="status" className="text-sm text-muted-foreground">
              {t('limitReached')}
            </p>
          ) : null}
          {result.recordsAvailable === 0 ? <p className="text-sm text-muted-foreground">{t('empty')}</p> : null}
          <div className="grid gap-3 sm:grid-cols-2">
            <Mix title={t('before')} values={result.before} />
            <Mix title={t('after')} values={result.after} />
          </div>
          <div
            className="max-h-80 max-w-full overflow-auto rounded-md border"
            tabIndex={0}
            aria-label={t('transitionTable')}
          >
            <table className="min-w-[620px] text-left text-sm">
              <thead className="sticky top-0 bg-muted">
                <tr>
                  <th scope="col" className="p-2">
                    {t('before')}
                  </th>
                  <th scope="col" className="p-2">
                    {t('after')}
                  </th>
                  <th scope="col" className="p-2">
                    {t('count')}
                  </th>
                  <th scope="col" className="p-2">
                    {t('changedRecords')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {result.transitions.map((cell) => (
                  <tr key={`${cell.before}\u0000${cell.after}`} className="border-t align-top">
                    <th scope="row" className="p-2 font-medium">
                      {cell.before}
                    </th>
                    <td className="p-2">{cell.after}</td>
                    <td className="p-2 tabular-nums">{cell.count}</td>
                    <td className="max-w-64 p-2 font-mono text-xs">
                      <ul className="grid gap-1">
                        {cell.records.map((id) => (
                          <li key={id}>{id}</li>
                        ))}
                      </ul>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {Object.keys(result.uncomparable).length > 0 ? (
            <div className="grid gap-1 text-sm">
              <h3 className="font-medium">{t('uncomparable')}</h3>
              {Object.entries(result.uncomparable).map(([reason, count]) => (
                <p key={reason} className="text-muted-foreground">
                  {t(`reason.${reason}` as 'reason.contractRejected', { count })}
                </p>
              ))}
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}

function Mix({ title, values }: { title: string; values: Record<string, number> }) {
  return (
    <div>
      <h3 className="mb-1 text-sm font-medium">{title}</h3>
      <ul className="grid gap-1 text-sm">
        {Object.entries(values).map(([outcome, count]) => (
          <li key={outcome} className="flex justify-between gap-3">
            <span className="break-all">{outcome}</span>
            <span className="tabular-nums">{count}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
