'use client';

import { useForm } from '@tanstack/react-form';
import { GitCompareArrows, History, MoreHorizontal, RotateCcw, Save } from 'lucide-react';
import dynamic from 'next/dynamic';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { When } from '@/components/shared/format';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { FieldGroup } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import { Skeleton } from '@/components/ui/skeleton';
import { Spinner } from '@/components/ui/spinner';

import type { DecisionNodeLabels } from './decision-node';
import { restoreVersion, saveVersion } from './decision.service';
import { MESSAGE_MAX, saveVersionSchema, type Decision, type SaveVersionValues, type Version } from './schema';
import { useVersion, useVersions, useVersionsChanged } from './useDecisions';
import type { useDraft } from './useDraft';

type Draft = ReturnType<typeof useDraft>;

const JdmDiffGraph = dynamic(() => import('./jdm-graph').then((module) => module.JdmDiffGraph), {
  ssr: false,
  loading: () => <Skeleton className="size-full rounded-lg" />,
});

interface VersionsProps {
  projectId: string;
  decision: Decision;
  draft: Draft;
  editable: boolean;
  callable: string[];
  decisionNodeLabels: DecisionNodeLabels;
}

/** A decision's place in its history: "v3", and whether the draft changed since. */
export function VersionChip({ latest, changed }: { latest: number | null; changed: boolean }) {
  const t = useTranslations('versions');
  if (latest === null) return <Badge variant="outline">{t('none')}</Badge>;
  return (
    <Badge variant="outline" className="gap-1.5">
      v{latest}
      {changed ? <span className="text-muted-foreground">· {t('changedSince')}</span> : null}
    </Badge>
  );
}

/** Saves the draft as it is on screen as the next version, with a message. */
export function SaveVersionDialog({
  projectId,
  decision,
  draft,
}: Omit<VersionsProps, 'editable' | 'callable' | 'decisionNodeLabels'>) {
  const t = useTranslations('versions');
  const common = useTranslations('common');
  const changedVersions = useVersionsChanged(projectId, decision.id);
  const [open, setOpen] = useState(false);
  // Fixed when the dialog opens, so its title holds while it closes after the save.
  const [next, setNext] = useState(1);
  const [error, setError] = useState<ActionError | null>(null);
  const unchanged = draft.version.latest !== null && !draft.version.changed;

  const form = useForm({
    defaultValues: { message: '' } satisfies SaveVersionValues,
    validators: { onChange: saveVersionSchema, onSubmit: saveVersionSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      // The version is exactly what is on screen: pending autosaves go first.
      if (!(await draft.settle())) {
        setError({ code: 'DECISION_CONFLICT' });
        return;
      }
      const result = await saveVersion(projectId, decision.id, value.message.trim(), draft.currentRevision());
      if (result.ok) {
        draft.versionSaved(result.data.number);
        changedVersions();
        toast.success(t('saved', { number: result.data.number }));
        onOpenChange(false);
      } else {
        setError(result.error);
      }
    },
  });

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      form.reset();
      setError(null);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <Button
        size="sm"
        disabled={unchanged}
        onClick={() => {
          setNext((draft.version.latest ?? 0) + 1);
          setOpen(true);
        }}
        title={unchanged ? t('unchanged') : undefined}
      >
        <Save aria-hidden />
        {t('save')}
      </Button>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('saveTitle', { number: next })}</DialogTitle>
          <DialogDescription>{t('saveDescription')}</DialogDescription>
        </DialogHeader>
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          <FieldGroup className="gap-2">
            <form.Field name="message">
              {(field) => (
                <TextField
                  field={field}
                  label={t('message')}
                  hint={t('messageHint')}
                  multiline
                  required
                  autoFocus
                  messageValues={{ max: MESSAGE_MAX }}
                />
              )}
            </form.Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton pending={isSubmitting} pendingLabel={t('saving')}>
                  {t('save')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** The decision's versions, newest first, to compare and restore. */
export function HistorySheet(props: VersionsProps) {
  const { projectId, decision, editable } = props;
  const t = useTranslations('versions');
  const common = useTranslations('common');
  const [open, setOpen] = useState(false);
  const history = useVersions(projectId, decision.id, open);
  const [compare, setCompare] = useState<{ from: number; to: number | 'draft' } | null>(null);
  const [restoring, setRestoring] = useState<Version | null>(null);
  const pages = history.data?.pages ?? [];
  const failed = pages.find((page) => !page.ok);
  const versions = pages.flatMap((page) => (page.ok ? page.data.items : []));

  return (
    <>
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetTrigger asChild>
          <Button variant="outline" size="sm">
            <History aria-hidden />
            {t('history')}
          </Button>
        </SheetTrigger>
        <SheetContent className="w-full gap-0 sm:max-w-md">
          <SheetHeader className="border-b">
            <SheetTitle>{t('historyTitle')}</SheetTitle>
            <SheetDescription className="font-mono">{decision.key}</SheetDescription>
          </SheetHeader>
          <div className="grid flex-1 content-start gap-2 overflow-y-auto p-4">
            {failed && !failed.ok ? (
              <ErrorAlert
                error={failed.error}
                action={
                  <Button variant="outline" size="sm" className="mt-2" onClick={() => void history.refetch()}>
                    {common('retry')}
                  </Button>
                }
              />
            ) : !history.data ? (
              [0, 1, 2].map((index) => <Skeleton key={index} className="h-16 w-full" />)
            ) : versions.length === 0 ? (
              <Empty>
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <History />
                  </EmptyMedia>
                  <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
                  <EmptyDescription>{editable ? t('empty') : t('emptyReadOnly')}</EmptyDescription>
                </EmptyHeader>
              </Empty>
            ) : (
              <ol className="grid gap-2">
                {versions.map((version) => (
                  <li key={version.number} className="grid gap-1 rounded-lg border p-3">
                    <div className="flex items-center gap-2">
                      <Badge variant="secondary">v{version.number}</Badge>
                      <span className="min-w-0 flex-1 truncate text-sm text-muted-foreground">
                        {version.createdBy.email} · <When value={version.createdAt} as="ago" />
                      </span>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={t('compareWithDraft', { number: version.number })}
                        title={t('compareWithDraft', { number: version.number })}
                        onClick={() => setCompare({ from: version.number, to: 'draft' })}
                      >
                        <GitCompareArrows aria-hidden />
                      </Button>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={t('actionsFor', { number: version.number })}
                          >
                            <MoreHorizontal aria-hidden />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onSelect={() => setCompare({ from: version.number, to: 'draft' })}>
                            <GitCompareArrows aria-hidden />
                            {t('compareWithDraft', { number: version.number })}
                          </DropdownMenuItem>
                          {version.number > 1 ? (
                            <DropdownMenuItem
                              onSelect={() => setCompare({ from: version.number - 1, to: version.number })}
                            >
                              <GitCompareArrows aria-hidden />
                              {t('compareWithPrevious', { number: version.number - 1 })}
                            </DropdownMenuItem>
                          ) : null}
                          {editable ? (
                            <DropdownMenuItem onSelect={() => setRestoring(version)}>
                              <RotateCcw aria-hidden />
                              {t('restore')}
                            </DropdownMenuItem>
                          ) : null}
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                    {/* A restore's message is the server's; it reads in the person's language here. */}
                    <p className="text-sm break-words whitespace-pre-line">
                      {version.restoredFrom ? t('restoredFrom', { number: version.restoredFrom }) : version.message}
                    </p>
                  </li>
                ))}
              </ol>
            )}
            {history.hasNextPage ? (
              <Button
                variant="ghost"
                size="sm"
                disabled={history.isFetchingNextPage}
                onClick={() => void history.fetchNextPage()}
              >
                {history.isFetchingNextPage ? <Spinner /> : null}
                {t('loadMore')}
              </Button>
            ) : null}
          </div>
        </SheetContent>
      </Sheet>
      {compare ? (
        <CompareDialog
          {...props}
          versions={versions}
          value={compare}
          onChange={setCompare}
          onClose={() => setCompare(null)}
        />
      ) : null}
      {restoring ? <RestoreDialog {...props} version={restoring} onClose={() => setRestoring(null)} /> : null}
    </>
  );
}

const DRAFT = 'draft';

/** Two versions (or a version and the draft) side by side as one diff graph. */
function CompareDialog({
  projectId,
  decision,
  draft,
  callable,
  decisionNodeLabels,
  versions,
  value,
  onChange,
  onClose,
}: VersionsProps & {
  versions: Version[];
  value: { from: number; to: number | 'draft' };
  onChange: (value: { from: number; to: number | 'draft' }) => void;
  onClose: () => void;
}) {
  const t = useTranslations('versions');
  const from = useVersion(projectId, decision.id, value.from);
  const to = useVersion(projectId, decision.id, value.to === DRAFT ? null : value.to);
  const previous = from.data?.ok ? from.data.data.content : undefined;
  const current = value.to === DRAFT ? draft.graph : to.data?.ok ? to.data.data.content : undefined;
  const failed = [from.data, to.data].find((result) => result && !result.ok);

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent className="flex h-[90dvh] w-[calc(100vw-2rem)] max-w-none flex-col gap-3 sm:max-w-[min(96vw,1400px)]">
        <DialogHeader>
          <DialogTitle>{t('compareTitle')}</DialogTitle>
          <DialogDescription>{t('compareDescription')}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <Select value={String(value.from)} onValueChange={(next) => onChange({ ...value, from: Number(next) })}>
            <SelectTrigger size="sm" className="w-40" aria-label={t('from')}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {versions.map((version) => (
                <SelectItem key={version.number} value={String(version.number)}>
                  v{version.number}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span aria-hidden>→</span>
          <Select
            value={String(value.to)}
            onValueChange={(next) => onChange({ ...value, to: next === DRAFT ? DRAFT : Number(next) })}
          >
            <SelectTrigger size="sm" className="w-40" aria-label={t('to')}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={DRAFT}>{t('draft')}</SelectItem>
              {versions.map((version) => (
                <SelectItem key={version.number} value={String(version.number)}>
                  v{version.number}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span className="ml-auto flex flex-wrap gap-3 text-xs text-muted-foreground">
            <Legend className="bg-green-500">{t('added')}</Legend>
            <Legend className="bg-red-500">{t('removed')}</Legend>
            <Legend className="bg-amber-500">{t('modified')}</Legend>
          </span>
        </div>
        <div className="min-h-0 flex-1 overflow-hidden rounded-lg border">
          {failed && !failed.ok ? (
            <div className="p-4">
              <ErrorAlert error={failed.error} />
            </div>
          ) : previous !== undefined && current !== undefined ? (
            <JdmDiffGraph
              current={current}
              previous={previous}
              callable={callable}
              decisionNodeLabels={decisionNodeLabels}
            />
          ) : (
            <Skeleton className="size-full" />
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t('close')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Legend({ className, children }: { className: string; children: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span aria-hidden className={`size-2 rounded-full ${className}`} />
      {children}
    </span>
  );
}

/** Restoring puts an old version back in the draft and adds it as a new version. */
function RestoreDialog({
  projectId,
  decision,
  draft,
  version,
  onClose,
}: VersionsProps & { version: Version; onClose: () => void }) {
  const t = useTranslations('versions');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changedVersions = useVersionsChanged(projectId, decision.id);
  const [pending, setPending] = useState(false);
  // The version the restore will add, as it was when the dialog opened.
  const [next] = useState(() => (draft.version.latest ?? 0) + 1);

  async function restore(number: number) {
    setPending(true);
    // The draft on the server must be what this person sees before it is replaced.
    const result = (await draft.settle())
      ? await restoreVersion(projectId, decision.id, number, draft.currentRevision())
      : ({ ok: false, error: { code: 'DECISION_CONFLICT' } } as const);
    setPending(false);
    if (!result.ok) {
      toast.error(errors(result.error.code));
      return;
    }
    draft.replace(result.data.decision);
    changedVersions();
    toast.success(t('restored', { from: number, number: result.data.version.number }));
    onClose();
  }

  return (
    <AlertDialog open onOpenChange={(open) => (open ? null : onClose())}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t('restoreTitle', { number: version.number })}</AlertDialogTitle>
          <AlertDialogDescription>{t('restoreConfirm', { number: next })}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={pending}>{common('cancel')}</AlertDialogCancel>
          <AlertDialogAction
            disabled={pending}
            onClick={(event) => {
              // Stay open while restoring; close when done.
              event.preventDefault();
              void restore(version.number);
            }}
          >
            {pending ? <Spinner /> : null}
            {t('restore')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
