'use client';

import { CheckCheck, TableProperties } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
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
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import type { Project } from '@/modules/project';

import { acknowledgeFinding } from './decision.service';
import { ACKNOWLEDGE_MAX, acknowledgeSchema, type AcknowledgeValues, type TableFinding } from './schema';
import { useFindings, useFindingsChanged } from './useDecisions';

/** Owners of an active project acknowledge a finding as intended. */
export function canAcknowledge(project: Project): boolean {
  return project.role === 'owner' && !project.archivedAt;
}

/** Findings nobody has acknowledged yet: what still needs a look. */
export function openFindings(findings: TableFinding[]): TableFinding[] {
  return findings.filter((finding) => !finding.acknowledgement);
}

/**
 * The editor's "Table checks": gaps, rows that are never used and conditions that never match in
 * the draft's decision tables, read again after every save (DNK-76).
 */
export function TableChecksSheet({
  project,
  decisionId,
  savedAt,
  open,
  onOpenChange,
}: {
  project: Project;
  decisionId: string;
  /** When the draft was last saved; null before the first save of this session. */
  savedAt: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const t = useTranslations('tableChecks');
  const findings = useFindings(project.id, decisionId, savedAt).data;
  const items = findings?.ok ? findings.data : [];
  const waiting = openFindings(items).length;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetTrigger asChild>
        <Button variant="outline">
          <TableProperties aria-hidden />
          {t('button')}
          {waiting > 0 ? (
            <Badge variant="secondary" aria-label={t('open', { count: waiting })}>
              {waiting}
            </Badge>
          ) : null}
        </Button>
      </SheetTrigger>
      <SheetContent className="w-full overflow-y-auto sm:max-w-xl">
        <SheetHeader>
          <SheetTitle>{t('title')}</SheetTitle>
          <SheetDescription>{t('description')}</SheetDescription>
        </SheetHeader>
        <div className="grid gap-3 px-4 pb-4">
          {findings && !findings.ok ? <ErrorAlert error={findings.error} /> : null}
          {findings?.ok && items.length === 0 ? (
            <Empty className="border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <CheckCheck />
                </EmptyMedia>
                <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
                <EmptyDescription>{t('empty')}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : null}
          <FindingList
            findings={items}
            acknowledge={canAcknowledge(project) ? { projectId: project.id, decisionId } : undefined}
          />
        </div>
      </SheetContent>
    </Sheet>
  );
}

/** The findings, each in words with its acknowledgement; owners acknowledge the open ones. */
export function FindingList({
  findings,
  acknowledge,
}: {
  findings: TableFinding[];
  acknowledge?: { projectId: string; decisionId: string };
}) {
  const t = useTranslations('tableChecks');
  const [acknowledging, setAcknowledging] = useState<TableFinding | null>(null);
  if (findings.length === 0) return null;
  return (
    <>
      <ul className="grid gap-2" aria-label={t('title')}>
        {findings.map((finding) => (
          <li key={finding.fingerprint} className="grid gap-1 rounded-md border p-3 text-sm">
            <div className="flex flex-wrap items-start justify-between gap-2">
              <FindingText finding={finding} />
              {acknowledge && !finding.acknowledgement ? (
                <Button variant="outline" size="sm" onClick={() => setAcknowledging(finding)}>
                  {t('acknowledge')}
                </Button>
              ) : null}
            </div>
            {finding.acknowledgement ? (
              <p className="text-muted-foreground">
                <Badge variant="outline" className="me-2">
                  {t('acknowledgedBadge')}
                </Badge>
                {t('acknowledged', {
                  person: finding.acknowledgement.acknowledgedBy.email,
                  reason: finding.acknowledgement.reason,
                })}
              </p>
            ) : null}
          </li>
        ))}
      </ul>
      {acknowledge && acknowledging ? (
        <AcknowledgeDialog {...acknowledge} finding={acknowledging} onClose={() => setAcknowledging(null)} />
      ) : null}
    </>
  );
}

/** One finding in words: which table, which row and column, and an input that shows it. */
export function FindingText({ finding }: { finding: TableFinding }) {
  const t = useTranslations('tableChecks');
  const table = finding.tableName || t('unnamedTable');
  const rows = (numbers: number[]) => numbers.join(', ');
  switch (finding.kind) {
    case 'gap':
      return (
        <div className="grid gap-1">
          <p className="font-medium">{t('gap', { table, count: finding.caseCount ?? finding.cases.length })}</p>
          <ul className="grid gap-1 ps-4">
            {finding.cases.map((gap, index) => (
              <li key={index} className="list-disc">
                <span className="font-mono text-xs">
                  {gap.conditions.length > 0
                    ? gap.conditions.map((c) => `${c.column.name} ${c.condition}`).join(' · ')
                    : t('anyInput')}
                </span>
                {gap.example ? <Example input={gap.example} /> : null}
              </li>
            ))}
          </ul>
          {(finding.caseCount ?? 0) > finding.cases.length ? (
            <p className="text-muted-foreground">
              {t('moreCases', { count: (finding.caseCount ?? 0) - finding.cases.length })}
            </p>
          ) : null}
        </div>
      );
    case 'overlap':
      return (
        <div className="grid gap-1">
          <p className="font-medium">
            {t(finding.coveredBy.length > 1 ? 'overlapMany' : 'overlapOne', {
              table,
              row: finding.row ?? 0,
              rows: rows(finding.coveredBy),
            })}
          </p>
          {finding.sameResult ? <p>{t('sameResult')}</p> : null}
          {finding.also.length > 0 ? (
            <p>{t(finding.also.length > 1 ? 'alsoMany' : 'alsoOne', { rows: rows(finding.also) })}</p>
          ) : null}
          {finding.example ? <Example input={finding.example} /> : null}
        </div>
      );
    case 'neverMatches':
      return (
        <p className="font-medium">
          {t('neverMatches', { table, row: finding.row ?? 0, column: finding.column?.name ?? '' })}
        </p>
      );
    case 'incomplete':
      return <p className="font-medium">{t('incomplete', { table })}</p>;
  }
}

function Example({ input }: { input: unknown }) {
  const t = useTranslations('tableChecks');
  return (
    <p className="text-muted-foreground">
      {t('example')} <code className="font-mono text-xs break-all">{JSON.stringify(input)}</code>
    </p>
  );
}

function AcknowledgeDialog({
  projectId,
  decisionId,
  finding,
  onClose,
}: {
  projectId: string;
  decisionId: string;
  finding: TableFinding;
  onClose: () => void;
}) {
  const t = useTranslations('tableChecks');
  const common = useTranslations('common');
  const changed = useFindingsChanged(projectId, decisionId);
  const [error, setError] = useState<ActionError | null>(null);

  const form = useAppForm({
    defaultValues: { reason: '' } satisfies AcknowledgeValues,
    validators: { onChange: acknowledgeSchema, onSubmit: acknowledgeSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await acknowledgeFinding(projectId, decisionId, finding.fingerprint, value.reason.trim());
      changed();
      if (!result.ok) {
        setError(result.error);
        return;
      }
      onClose();
      toast.success(t('acknowledgedToast'));
    },
  });

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('acknowledgeTitle')}</DialogTitle>
          <DialogDescription>{t('acknowledgeDescription')}</DialogDescription>
        </DialogHeader>
        <div className="rounded-md border p-3 text-sm">
          <FindingText finding={finding} />
        </div>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-2">
              <form.AppField name="reason">
                {(field) => (
                  <field.TextField
                    label={t('reason')}
                    hint={t('reasonHint')}
                    multiline
                    rows={3}
                    required
                    autoFocus
                    messageValues={{ max: ACKNOWLEDGE_MAX }}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('acknowledging')}>{t('acknowledge')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
