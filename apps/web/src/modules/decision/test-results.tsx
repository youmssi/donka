'use client';

import { CircleAlert, CircleCheck, CircleX } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { toast } from 'sonner';

import { ErrorAlert } from '@/components/shared/error-alert';
import { cn } from '@/components/shared/utils';
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
import { Item, ItemGroup } from '@/components/ui/item';
import { Skeleton } from '@/components/ui/skeleton';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

import type { TestResult, TestSummary } from './schema';
import { useTestResults } from './useDecisions';

/** "3 passed", or how many of the scenarios fail; nothing when none ran. */
export function TestBadge({ summary, onClick }: { summary: TestSummary; onClick?: () => void }) {
  const t = useTranslations('tests');
  const total = summary.passed + summary.failed + summary.errors;
  if (total === 0) return null;
  const failing = summary.failed + summary.errors;
  const label = t('summary', { passed: summary.passed, failed: summary.failed, errors: summary.errors });
  const content = failing ? (
    <>
      <CircleX aria-hidden />
      {t('failing', { failing, total })}
    </>
  ) : (
    <>
      <CircleCheck aria-hidden />
      {t('passing', { total })}
    </>
  );
  const className = cn(
    failing
      ? 'border-destructive/40 text-destructive'
      : 'border-emerald-600/40 text-emerald-700 dark:border-emerald-400/40 dark:text-emerald-400',
  );
  return onClick ? (
    <Badge asChild variant="outline" className={className}>
      <button type="button" onClick={onClick} aria-label={label} title={label}>
        {content}
      </button>
    </Badge>
  ) : (
    <Badge variant="outline" className={className} aria-label={label} title={label}>
      {content}
    </Badge>
  );
}

const ORDER = { failed: 0, error: 1, passed: 2 } as const;

/** Every scenario as it ran on one version; failures show expected and actual per field. */
export function TestResultsDialog({
  projectId,
  decisionId,
  number,
  onClose,
}: {
  projectId: string;
  decisionId: string;
  number: number;
  onClose: () => void;
}) {
  const t = useTranslations('tests');
  const common = useTranslations('common');
  const query = useTestResults(projectId, decisionId, number);
  const result = query.data;
  const items = result?.ok ? [...result.data.items].sort((a, b) => ORDER[a.status] - ORDER[b.status]) : [];

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent className="max-h-[90dvh] grid-rows-[auto_1fr_auto] sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{t('title', { number })}</DialogTitle>
          <DialogDescription>
            {result?.ok
              ? t('summary', {
                  passed: result.data.summary.passed,
                  failed: result.data.summary.failed,
                  errors: result.data.summary.errors,
                })
              : t('description')}
          </DialogDescription>
        </DialogHeader>
        <div className="-mx-6 grid content-start gap-3 overflow-y-auto px-6">
          {!result ? (
            [0, 1, 2].map((index) => <Skeleton key={index} className="h-14 w-full" />)
          ) : !result.ok ? (
            <ErrorAlert
              error={result.error}
              action={
                <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
                  {common('retry')}
                </Button>
              }
            />
          ) : items.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('none')}</p>
          ) : (
            <ItemGroup className="gap-3">
              {items.map((item) => (
                <ResultItem key={item.scenarioId} result={item} />
              ))}
            </ItemGroup>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {common('close')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ResultItem({ result }: { result: TestResult }) {
  const t = useTranslations('tests');
  const scenarios = useTranslations('scenarios');
  const Icon = result.status === 'passed' ? CircleCheck : result.status === 'failed' ? CircleX : CircleAlert;
  return (
    <Item variant="outline" size="sm" role="listitem" className="flex-col items-stretch gap-2">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <Icon
          className={cn(
            'size-4 shrink-0',
            result.status === 'passed' ? 'text-emerald-600 dark:text-emerald-400' : 'text-destructive',
          )}
          aria-hidden
        />
        <span className="font-medium">{result.name}</span>
        <span className="font-mono text-xs text-muted-foreground">{result.decisionKey}</span>
        <Badge variant="secondary" className="ml-auto">
          {scenarios(result.match)}
        </Badge>
        <span className="sr-only">{t(result.status)}</span>
      </div>
      {result.status === 'error' ? (
        <p className="text-sm text-destructive">
          {result.missingDecision
            ? t('noVersion', { key: result.missingDecision })
            : t('couldNotRun', { reason: result.error ?? t('engineFailed') })}
        </p>
      ) : null}
      {result.status === 'failed' ? (
        <Table className="text-xs">
          <TableHeader>
            <TableRow>
              <TableHead>{t('field')}</TableHead>
              <TableHead>{t('expected')}</TableHead>
              <TableHead>{t('actual')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {result.mismatches.map((mismatch) => (
              <TableRow key={mismatch.path}>
                <TableCell className="font-mono">{mismatch.path || t('wholeOutput')}</TableCell>
                <TableCell className="font-mono break-all whitespace-normal">
                  <Value value={mismatch.expected} />
                </TableCell>
                <TableCell className="font-mono break-all whitespace-normal">
                  <Value value={mismatch.actual} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}
    </Item>
  );
}

/** A JSON value in one line; a missing one says so. */
function Value({ value }: { value: unknown }) {
  const t = useTranslations('tests');
  return value === undefined ? (
    <span className="text-muted-foreground italic">{t('missing')}</span>
  ) : (
    <>{JSON.stringify(value)}</>
  );
}

/** Says a version was saved, and how its scenarios went when there are any. */
export function useAnnounceVersion() {
  const t = useTranslations('tests');
  return (title: string, summary: TestSummary) => {
    if (summary.passed + summary.failed + summary.errors === 0) {
      toast.success(title);
      return;
    }
    const description = t('summary', { passed: summary.passed, failed: summary.failed, errors: summary.errors });
    if (summary.failed + summary.errors > 0) toast.warning(title, { description });
    else toast.success(title, { description });
  };
}
