'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Field, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { useModule } from '@/modules/licence';
import { ProjectFrame, type Project } from '@/modules/project';

import type { Band, MixWeek, Shift, Snapshot } from './schema';
import { listSnapshots, takeSnapshot } from './stability.service';

export function StabilityPage() {
  return <ProjectFrame section="stability">{(project) => <Stability project={project} />}</ProjectFrame>;
}

/** A date input's value, `YYYY-MM-DD`, for a date `days` before now. */
function daysAgo(days: number) {
  const date = new Date();
  date.setUTCDate(date.getUTCDate() - days);
  return date.toISOString().slice(0, 10);
}

function startOfDay(value: string) {
  return `${value}T00:00:00.000Z`;
}

function endOfDay(value: string) {
  return `${value}T23:59:59.999Z`;
}

/** The badge tone for a band. A breach is destructive; "moved" is a warning, not a failure. */
function bandVariant(band: Band): 'secondary' | 'outline' | 'destructive' {
  switch (band) {
    case 'shifted':
      return 'destructive';
    case 'moved':
      return 'outline';
    default:
      return 'secondary';
  }
}

function Stability({ project }: { project: Project }) {
  const t = useTranslations('stability');
  const licensed = useModule('analysis');
  const owner = project.role === 'owner';
  const searchParams = useSearchParams();

  const [decisionKey, setDecisionKey] = useState(searchParams.get('decision') ?? '');
  const [outputField, setOutputField] = useState('');
  const [outputBins, setOutputBins] = useState('');
  const [inputFields, setInputFields] = useState('');
  // A quarter of reference against the last month is the comparison risk teams start from.
  const [referenceFrom, setReferenceFrom] = useState(daysAgo(120));
  const [referenceUntil, setReferenceUntil] = useState(daysAgo(31));
  const [recentFrom, setRecentFrom] = useState(daysAgo(30));
  const [recentUntil, setRecentUntil] = useState(daysAgo(0));
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [error, setError] = useState<ActionError | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!owner || !licensed || !decisionKey) return;
    let cancelled = false;
    void listSnapshots(project.id, decisionKey).then((result) => {
      if (cancelled) return;
      if (result.ok) setSnapshots(result.data);
      else setError(result.error);
    });
    return () => {
      cancelled = true;
    };
  }, [decisionKey, licensed, owner, project.id]);

  async function run() {
    setBusy(true);
    setError(null);
    const bins = outputBins
      .split(',')
      .map((edge) => Number(edge.trim()))
      .filter((edge) => Number.isFinite(edge));
    const result = await takeSnapshot(project.id, {
      decisionKey,
      referenceFrom: startOfDay(referenceFrom),
      referenceUntil: endOfDay(referenceUntil),
      recentFrom: startOfDay(recentFrom),
      recentUntil: endOfDay(recentUntil),
      outputField: outputField.trim() || undefined,
      outputBins: bins,
      inputFields: inputFields
        .split(',')
        .map((field) => field.trim())
        .filter(Boolean),
    });
    if (result.ok) setSnapshots([result.data, ...snapshots]);
    else setError(result.error);
    setBusy(false);
  }

  if (!owner || !licensed) return null;

  const latest = snapshots[0];

  return (
    <div className="grid min-w-0 gap-4">
      <Card>
        <CardHeader>
          <CardTitle>{t('title')}</CardTitle>
          <CardDescription>{t('description')}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <p className="text-sm text-muted-foreground">{t('limitation')}</p>
          {error ? <ErrorAlert error={error} /> : null}
          <div className="grid gap-3 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="stability-decision">{t('decision')}</FieldLabel>
              <Input
                id="stability-decision"
                value={decisionKey}
                onChange={(event) => setDecisionKey(event.target.value)}
                disabled={busy}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-output">{t('outputField')}</FieldLabel>
              <Input
                id="stability-output"
                value={outputField}
                onChange={(event) => setOutputField(event.target.value)}
                disabled={busy}
                placeholder={t('outputFieldHint')}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-bins">{t('outputBins')}</FieldLabel>
              <Input
                id="stability-bins"
                value={outputBins}
                onChange={(event) => setOutputBins(event.target.value)}
                disabled={busy}
                placeholder="500, 600, 700"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-inputs">{t('inputFields')}</FieldLabel>
              <Input
                id="stability-inputs"
                value={inputFields}
                onChange={(event) => setInputFields(event.target.value)}
                disabled={busy}
                placeholder="applicant.region, applicant.employment"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-reference-from">{t('referenceFrom')}</FieldLabel>
              <Input
                id="stability-reference-from"
                type="date"
                value={referenceFrom}
                onChange={(event) => setReferenceFrom(event.target.value)}
                disabled={busy}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-reference-until">{t('referenceUntil')}</FieldLabel>
              <Input
                id="stability-reference-until"
                type="date"
                value={referenceUntil}
                onChange={(event) => setReferenceUntil(event.target.value)}
                disabled={busy}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-recent-from">{t('recentFrom')}</FieldLabel>
              <Input
                id="stability-recent-from"
                type="date"
                value={recentFrom}
                onChange={(event) => setRecentFrom(event.target.value)}
                disabled={busy}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="stability-recent-until">{t('recentUntil')}</FieldLabel>
              <Input
                id="stability-recent-until"
                type="date"
                value={recentUntil}
                onChange={(event) => setRecentUntil(event.target.value)}
                disabled={busy}
              />
            </Field>
          </div>
          <div>
            <Button onClick={() => void run()} disabled={busy || !decisionKey}>
              {t('run')}
            </Button>
          </div>
        </CardContent>
      </Card>

      {latest ? <SnapshotResult snapshot={latest} /> : null}
      {snapshots.length > 1 ? <Earlier snapshots={snapshots.slice(1)} /> : null}
    </div>
  );
}

function SnapshotResult({ snapshot }: { snapshot: Snapshot }) {
  const t = useTranslations('stability');
  return (
    <section aria-label={t('result')} className="grid min-w-0 gap-4">
      <Card className="min-w-0">
        <CardHeader>
          <CardTitle className="flex flex-wrap items-center gap-2">
            {t('resultTitle')}
            <Badge variant={bandVariant(snapshot.worstBand)}>{t(`band.${snapshot.worstBand}`)}</Badge>
          </CardTitle>
          <CardDescription>
            {t('measuredAgainst', {
              stableBelow: snapshot.thresholds.stableBelow,
              shiftedAt: snapshot.thresholds.shiftedAt,
              minimumBin: snapshot.minimumBin,
            })}
          </CardDescription>
        </CardHeader>
        <CardContent className="grid min-w-0 gap-4">
          <p className="text-sm">{t('recordsRead', { count: snapshot.recordsRead })}</p>
          <Mix weeks={snapshot.mix} />
          <Shifts shifts={snapshot.shifts} />
        </CardContent>
      </Card>
    </section>
  );
}

function Mix({ weeks }: { weeks: MixWeek[] }) {
  const t = useTranslations('stability');
  if (weeks.length === 0) return <p className="text-sm text-muted-foreground">{t('noMix')}</p>;
  const outcomes = [...new Set(weeks.flatMap((week) => Object.keys(week.shares)))].sort();
  return (
    <div className="min-w-0">
      <h3 className="mb-1 text-sm font-medium">{t('mix')}</h3>
      {/* The table scrolls inside its own area, so the page never scrolls sideways at 390 px. */}
      <div className="max-w-full overflow-x-auto rounded-md border" tabIndex={0} aria-label={t('mix')}>
        <table className="min-w-[420px] text-left text-sm">
          <thead className="bg-muted">
            <tr>
              <th scope="col" className="p-2">
                {t('week')}
              </th>
              <th scope="col" className="p-2">
                {t('records')}
              </th>
              {outcomes.map((outcome) => (
                <th scope="col" key={outcome} className="p-2">
                  {outcome}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {weeks.map((week) => (
              <tr key={week.week} className="border-t">
                <th scope="row" className="p-2 font-medium">
                  {week.week}
                </th>
                <td className="p-2 tabular-nums">{week.records}</td>
                {outcomes.map((outcome) => (
                  <td key={outcome} className="p-2 tabular-nums">
                    {week.shares[outcome] === undefined ? '—' : `${(week.shares[outcome] * 100).toFixed(1)}%`}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function Shifts({ shifts }: { shifts: Shift[] }) {
  const t = useTranslations('stability');
  if (shifts.length === 0) return <p className="text-sm text-muted-foreground">{t('noShifts')}</p>;
  return (
    <div className="min-w-0">
      <h3 className="mb-1 text-sm font-medium">{t('shifts')}</h3>
      <div className="max-w-full overflow-x-auto rounded-md border" tabIndex={0} aria-label={t('shifts')}>
        <table className="min-w-[520px] text-left text-sm">
          <thead className="bg-muted">
            <tr>
              <th scope="col" className="p-2">
                {t('field')}
              </th>
              <th scope="col" className="p-2">
                {t('index')}
              </th>
              <th scope="col" className="p-2">
                {t('bandColumn')}
              </th>
              <th scope="col" className="p-2">
                {t('periods')}
              </th>
              <th scope="col" className="p-2">
                {t('suppressed')}
              </th>
            </tr>
          </thead>
          <tbody>
            {shifts.map((shift) => (
              <tr key={shift.field} className="border-t align-top">
                <th scope="row" className="p-2 font-medium break-all">
                  {shift.field}
                </th>
                <td className="p-2 tabular-nums">
                  {/* A period that cannot answer says so rather than showing a number that
                      would be read as "stable". */}
                  {shift.index === null || shift.index === undefined
                    ? t(`unmeasurable.${shift.reason ?? 'noComparableValues'}`)
                    : shift.index.toFixed(3)}
                </td>
                <td className="p-2">
                  {shift.band ? <Badge variant={bandVariant(shift.band)}>{t(`band.${shift.band}`)}</Badge> : '—'}
                </td>
                <td className="p-2 tabular-nums">
                  {shift.referenceRecords} / {shift.recentRecords}
                </td>
                <td className="p-2 tabular-nums">
                  {shift.suppressedBins > 0 ? t('suppressedBins', { count: shift.suppressedBins }) : '—'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function Earlier({ snapshots }: { snapshots: Snapshot[] }) {
  const t = useTranslations('stability');
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('earlier')}</CardTitle>
      </CardHeader>
      <CardContent>
        <ul className="grid gap-2 text-sm">
          {snapshots.map((snapshot) => (
            <li key={snapshot.id} className="flex flex-wrap items-center justify-between gap-2">
              <span>{new Date(snapshot.createdAt).toISOString().slice(0, 16).replace('T', ' ')}</span>
              <span className="flex items-center gap-2">
                <span className="text-muted-foreground tabular-nums">
                  {t('recordsRead', { count: snapshot.recordsRead })}
                </span>
                <Badge variant={bandVariant(snapshot.worstBand)}>{t(`band.${snapshot.worstBand}`)}</Badge>
              </span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  );
}
