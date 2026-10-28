'use client';

import { MessageSquareWarning } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { When } from '@/components/shared/format';
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
import { FieldGroup } from '@/components/ui/field';
import { Skeleton } from '@/components/ui/skeleton';
import type { Project } from '@/modules/project';

import {
  CONTEST_TEXT_MAX,
  contestSchema,
  REVISED_OUTCOME_MAX,
  reviewSchema,
  type Contest,
  type ContestStatus,
  type ContestValues,
  type ReviewValues,
} from './schema';
import { useContests, useOpenContest, useReviewContest } from './useDecisionLog';

/** The person's contests of this decision and what each review concluded: its history. */
export function ContestsCard({ project, recordId }: { project: Project; recordId: string }) {
  const t = useTranslations('contests');
  const query = useContests(project.id, recordId);
  const [dialog, setDialog] = useState<{ kind: 'open' } | { kind: 'review'; contest: Contest } | null>(null);
  const result = query.data;
  const owner = project.role === 'owner';
  const underReview = result?.ok ? result.data.some((contest) => contest.status === 'open') : true;

  return (
    <Card className="min-w-0">
      <CardHeader className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1.5">
          <CardTitle>{t('title')}</CardTitle>
          <CardDescription>{t('description')}</CardDescription>
        </div>
        {owner && !underReview ? (
          <Button variant="outline" size="sm" onClick={() => setDialog({ kind: 'open' })}>
            <MessageSquareWarning aria-hidden />
            {t('open')}
          </Button>
        ) : null}
      </CardHeader>
      <CardContent>
        {!result ? (
          <Skeleton className="h-12 w-full" />
        ) : !result.ok ? (
          <ErrorAlert error={result.error} />
        ) : result.data.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t('none')}</p>
        ) : (
          <ol className="grid gap-3" aria-label={t('title')}>
            {result.data.map((contest) => (
              <li key={contest.id} className="grid gap-2 rounded-lg border p-3 text-sm">
                <div className="flex flex-wrap items-center gap-2">
                  <StatusBadge status={contest.status} />
                  <span className="text-muted-foreground">
                    {t('openedOn')} <When value={contest.openedAt} as="dateTime" />
                  </span>
                </div>
                <p className="break-words whitespace-pre-wrap">{contest.reason}</p>
                {contest.review ? (
                  <div className="grid gap-1 border-l-2 pl-3">
                    <span className="text-xs text-muted-foreground">
                      {t('reviewedOn')} {contest.reviewedAt ? <When value={contest.reviewedAt} as="dateTime" /> : null}
                    </span>
                    <p className="break-words whitespace-pre-wrap">{contest.review}</p>
                    {contest.revisedOutcome ? (
                      <p>
                        {t('newOutcome')}{' '}
                        <Badge variant="secondary" className="font-mono">
                          {contest.revisedOutcome}
                        </Badge>
                      </p>
                    ) : null}
                  </div>
                ) : owner ? (
                  <Button
                    variant="outline"
                    size="sm"
                    className="w-fit"
                    onClick={() => setDialog({ kind: 'review', contest })}
                  >
                    {t('review')}
                  </Button>
                ) : null}
              </li>
            ))}
          </ol>
        )}
      </CardContent>
      {dialog?.kind === 'open' ? (
        <OpenDialog project={project} recordId={recordId} onClose={() => setDialog(null)} />
      ) : null}
      {dialog?.kind === 'review' ? (
        <ReviewDialog project={project} recordId={recordId} contest={dialog.contest} onClose={() => setDialog(null)} />
      ) : null}
    </Card>
  );
}

function StatusBadge({ status }: { status: ContestStatus }) {
  const t = useTranslations('contests');
  if (status === 'open') return <Badge>{t('statusOpen')}</Badge>;
  return <Badge variant="outline">{status === 'upheld' ? t('statusUpheld') : t('statusRevised')}</Badge>;
}

function OpenDialog({ project, recordId, onClose }: { project: Project; recordId: string; onClose: () => void }) {
  const t = useTranslations('contests');
  const common = useTranslations('common');
  const open = useOpenContest(project.id, recordId);
  const [error, setError] = useState<ActionError | null>(null);

  const defaultValues: ContestValues = { reason: '' };
  const form = useAppForm({
    defaultValues,
    validators: { onChange: contestSchema, onSubmit: contestSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await open.mutateAsync(value.reason.trim());
      if (!result.ok) {
        setError(result.error);
        return;
      }
      toast.success(t('opened'));
      onClose();
    },
  });

  return (
    <Dialog open onOpenChange={(next) => (next ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('openTitle')}</DialogTitle>
          <DialogDescription>{t('openDescription')}</DialogDescription>
        </DialogHeader>
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
                    rows={4}
                    required
                    autoFocus
                    messageValues={{ max: CONTEST_TEXT_MAX }}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('saving')}>{t('openSubmit')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}

function ReviewDialog({
  project,
  recordId,
  contest,
  onClose,
}: {
  project: Project;
  recordId: string;
  contest: Contest;
  onClose: () => void;
}) {
  const t = useTranslations('contests');
  const common = useTranslations('common');
  const review = useReviewContest(project.id, recordId);
  const [error, setError] = useState<ActionError | null>(null);

  const defaultValues: ReviewValues = { status: 'upheld', review: '', revisedOutcome: '' };
  const form = useAppForm({
    defaultValues,
    validators: { onChange: reviewSchema, onSubmit: reviewSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await review.mutateAsync({
        id: contest.id,
        review: { ...value, review: value.review.trim(), revisedOutcome: value.revisedOutcome.trim() },
      });
      if (!result.ok) {
        setError(result.error);
        return;
      }
      toast.success(t('reviewed'));
      onClose();
    },
  });

  return (
    <Dialog open onOpenChange={(next) => (next ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('reviewTitle')}</DialogTitle>
          <DialogDescription className="break-words">{contest.reason}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-2">
              <form.AppField name="status">
                {(field) => (
                  <field.SelectField
                    label={t('outcome')}
                    options={[
                      { value: 'upheld', label: t('statusUpheld') },
                      { value: 'revised', label: t('statusRevised') },
                    ]}
                  />
                )}
              </form.AppField>
              <form.Subscribe selector={(state) => state.values.status}>
                {(status) =>
                  status === 'revised' ? (
                    <form.AppField name="revisedOutcome">
                      {(field) => (
                        <field.TextField
                          label={t('newOutcomeLabel')}
                          hint={t('newOutcomeHint')}
                          required
                          messageValues={{ max: REVISED_OUTCOME_MAX }}
                        />
                      )}
                    </form.AppField>
                  ) : null
                }
              </form.Subscribe>
              <form.AppField name="review">
                {(field) => (
                  <field.TextField
                    label={t('reviewText')}
                    hint={t('reviewHint')}
                    multiline
                    rows={4}
                    required
                    messageValues={{ max: CONTEST_TEXT_MAX }}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('saving')}>{t('reviewSubmit')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
