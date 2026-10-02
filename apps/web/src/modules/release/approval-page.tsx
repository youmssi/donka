'use client';

import { useForm } from '@tanstack/react-form';
import { ArrowRight, Check, CircleAlert, GitCompareArrows, Info, TriangleAlert, Undo2, X } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { When } from '@/components/shared/format';
import { PageHeader } from '@/components/shared/layout/page-header';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
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
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from '@/components/ui/item';
import { Skeleton } from '@/components/ui/skeleton';
import { Link } from '@/i18n/navigation';
import { TestBadge, VersionDiffDialog } from '@/modules/decision';
import { useCurrentUser } from '@/modules/identity';
import { decisionHref, projectHref, ProjectFrame, type Project } from '@/modules/project';

import { ApprovalBadge } from './approvals-page';
import { approve, reject, withdraw } from './release.service';
import { REASON_MAX, rejectSchema, type ApprovalReview, type DecisionChange, type RejectValues } from './schema';
import { useApproval, useReleasesChanged } from './useReleases';

export function ApprovalPage() {
  return (
    <ProjectFrame section="approval" header={() => null}>
      {(project) => <Review project={project} />}
    </ProjectFrame>
  );
}

function Review({ project }: { project: Project }) {
  const t = useTranslations('approvals');
  const common = useTranslations('common');
  const id = useSearchParams().get('a');
  const query = useApproval(project.id, id);
  const result = query.data;

  if (!id || (result && !result.ok && result.error.code === 'APPROVAL_NOT_FOUND')) {
    return (
      <Alert>
        <CircleAlert aria-hidden />
        <AlertTitle>{t('notFoundTitle')}</AlertTitle>
        <AlertDescription>
          <Link href={projectHref('approvals', project.key)} className="underline underline-offset-4">
            {t('backToList')}
          </Link>
        </AlertDescription>
      </Alert>
    );
  }
  if (result && !result.ok) {
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
  if (!result) {
    return (
      <div className="grid gap-4">
        <Skeleton className="h-9 w-72" />
        <div className="grid gap-4 lg:grid-cols-3">
          <Skeleton className="h-80 lg:col-span-2" />
          <Skeleton className="h-80" />
        </div>
      </div>
    );
  }
  return <ReviewBody project={project} review={result.data} />;
}

function ReviewBody({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals');
  const me = useCurrentUser();
  const pending = review.status === 'pending';
  const mine = review.requestedBy.id === me.id;
  const failing = review.tests.failed + review.tests.errors;

  return (
    <div className="grid gap-4">
      <PageHeader
        title={t('reviewTitle', { version: review.releaseVersion })}
        badges={<ApprovalBadge status={review.status} />}
        actions={
          <div className="flex flex-wrap gap-2">
            {pending && mine ? <WithdrawButton project={project} review={review} /> : null}
            {review.canDecide ? (
              <>
                <RejectButton project={project} review={review} />
                <ApproveButton project={project} review={review} />
              </>
            ) : null}
          </div>
        }
      />
      <Outcome project={project} review={review} />
      {pending && !review.canDecide ? (
        <Alert>
          <Info aria-hidden />
          <AlertDescription>
            {review.releaseCreatedBy.id === me.id || mine ? t('authorCannotDecide') : t('waitingForOwner')}
          </AlertDescription>
        </Alert>
      ) : null}
      <div className="grid items-start gap-4 lg:grid-cols-3">
        <Changes project={project} review={review} />
        <div className="grid gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{t('notes')}</CardTitle>
              <CardDescription>{t('releasedBy', { email: review.releaseCreatedBy.email })}</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm break-words whitespace-pre-line">{review.releaseNotes}</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{t('tests')}</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-3 text-sm">
              {review.tests.passed + failing === 0 ? (
                <p className="text-muted-foreground">{t('noTests')}</p>
              ) : (
                <div>
                  <TestBadge summary={review.tests} />
                </div>
              )}
              {failing > 0 ? (
                <p className="flex gap-2 text-muted-foreground">
                  <TriangleAlert className="mt-0.5 size-4 shrink-0 text-amber-600" aria-hidden />
                  {t('failingTests')}
                </p>
              ) : null}
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{t('request')}</CardTitle>
            </CardHeader>
            <CardContent>
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
                <dt className="text-muted-foreground">{t('requestedBy')}</dt>
                <dd className="min-w-0 truncate">{review.requestedBy.email}</dd>
                <dt className="text-muted-foreground">{t('requestedAt')}</dt>
                <dd>
                  <When value={review.requestedAt} as="dateTime" />
                </dd>
                {review.decidedBy && review.decidedAt ? (
                  <>
                    <dt className="text-muted-foreground">{t('decidedBy')}</dt>
                    <dd className="min-w-0 truncate">{review.decidedBy.email}</dd>
                    <dt className="text-muted-foreground">{t('decidedAt')}</dt>
                    <dd>
                      <When value={review.decidedAt} as="dateTime" />
                    </dd>
                  </>
                ) : null}
              </dl>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

/** How a decided request ended, said once at the top. */
function Outcome({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals');
  const who = review.decidedBy?.email ?? '';
  if (review.status === 'approved') {
    return (
      <Alert>
        <Check aria-hidden />
        <AlertTitle>{t('approvedTitle', { email: who })}</AlertTitle>
        <AlertDescription>
          <Link href={projectHref('environments', project.key)} className="underline underline-offset-4">
            {t('seeProduction')}
          </Link>
        </AlertDescription>
      </Alert>
    );
  }
  if (review.status === 'rejected') {
    return (
      <Alert variant="destructive">
        <X aria-hidden />
        <AlertTitle>{t('rejectedTitle', { email: who })}</AlertTitle>
        <AlertDescription>
          <p className="break-words whitespace-pre-line">{review.reason}</p>
        </AlertDescription>
      </Alert>
    );
  }
  if (review.status === 'withdrawn') {
    return (
      <Alert>
        <Undo2 aria-hidden />
        <AlertTitle>{t('withdrawnTitle', { email: who })}</AlertTitle>
      </Alert>
    );
  }
  return null;
}

/** Each decision's version in production and in the release, changes first. */
function Changes({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals');
  const [comparing, setComparing] = useState<DecisionChange | null>(null);
  const changed = review.changes.filter((change) => change.change !== 'unchanged').length;

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle className="flex flex-wrap items-center gap-2">
          <span className="font-mono">{review.productionVersion ?? '—'}</span>
          <ArrowRight className="size-4 text-muted-foreground" aria-hidden />
          <span className="font-mono">{review.releaseVersion}</span>
        </CardTitle>
        <CardDescription>
          {review.productionVersion ? t('changesSummary', { count: changed }) : t('firstProduction')}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <ItemGroup className="divide-y rounded-lg border">
          {review.changes.map((change) => (
            <Item key={change.decisionId} size="sm" className="rounded-none">
              <ItemContent className="min-w-0">
                <ItemTitle className="w-full">
                  <Link
                    href={decisionHref(project.key, change.key)}
                    className="min-w-0 truncate font-mono underline-offset-4 hover:underline"
                  >
                    {change.key}
                  </Link>
                </ItemTitle>
                <ItemDescription className="flex flex-wrap items-center gap-2">
                  <ChangeBadge change={change.change} />
                  <span className="font-mono text-xs">
                    {change.fromVersion ? `v${change.fromVersion}` : '—'} →{' '}
                    {change.toVersion ? `v${change.toVersion}` : '—'}
                  </span>
                  {change.tests ? <TestBadge summary={change.tests} /> : null}
                </ItemDescription>
              </ItemContent>
              {change.change !== 'unchanged' ? (
                <ItemActions>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setComparing(change)}
                    aria-label={t('compareFor', { key: change.key })}
                  >
                    <GitCompareArrows aria-hidden />
                    <span className="hidden sm:inline">{t('compare')}</span>
                  </Button>
                </ItemActions>
              ) : null}
            </Item>
          ))}
        </ItemGroup>
      </CardContent>
      {comparing ? (
        <VersionDiffDialog
          projectId={project.id}
          decisionId={comparing.decisionId}
          title={comparing.key}
          from={comparing.fromVersion ?? null}
          to={comparing.toVersion ?? null}
          onClose={() => setComparing(null)}
        />
      ) : null}
    </Card>
  );
}

function ChangeBadge({ change }: { change: DecisionChange['change'] }) {
  const t = useTranslations('approvals');
  const className = {
    added: 'border-green-600/40 text-green-700 dark:text-green-400',
    changed: 'border-amber-500/50 text-amber-700 dark:text-amber-400',
    removed: 'border-red-600/40 text-red-700 dark:text-red-400',
    unchanged: 'text-muted-foreground',
  }[change];
  return (
    <Badge variant="outline" className={className}>
      {t(change)}
    </Badge>
  );
}

function ApproveButton({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [open, setOpen] = useState(false);

  async function onApprove() {
    const result = await approve(project.id, review.id);
    changed();
    if (result.ok) toast.success(t('approved', { version: review.releaseVersion }));
    else toast.error(errors(result.error.code));
  }

  return (
    <>
      <Button onClick={() => setOpen(true)}>
        <Check aria-hidden />
        {t('approve')}
      </Button>
      <AlertDialog open={open} onOpenChange={setOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('approveTitle', { version: review.releaseVersion })}</AlertDialogTitle>
            <AlertDialogDescription>{t('approveConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void onApprove()}>{t('approve')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function RejectButton({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals');
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="outline" onClick={() => setOpen(true)}>
        <X aria-hidden />
        {t('reject')}
      </Button>
      {open ? <RejectDialog project={project} review={review} onClose={() => setOpen(false)} /> : null}
    </>
  );
}

function RejectDialog({ project, review, onClose }: { project: Project; review: ApprovalReview; onClose: () => void }) {
  const t = useTranslations('approvals');
  const common = useTranslations('common');
  const changed = useReleasesChanged(project.id);
  const [error, setError] = useState<ActionError | null>(null);

  const form = useForm({
    defaultValues: { reason: '' } satisfies RejectValues,
    validators: { onChange: rejectSchema, onSubmit: rejectSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await reject(project.id, review.id, value.reason.trim());
      changed();
      if (!result.ok) {
        setError(result.error);
        return;
      }
      onClose();
      toast.success(t('rejected', { version: review.releaseVersion }));
    },
  });

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('rejectTitle', { version: review.releaseVersion })}</DialogTitle>
          <DialogDescription>{t('rejectDescription')}</DialogDescription>
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
            <form.Field name="reason">
              {(field) => (
                <TextField
                  field={field}
                  label={t('reason')}
                  hint={t('reasonHint')}
                  multiline
                  rows={4}
                  required
                  autoFocus
                  messageValues={{ max: REASON_MAX }}
                />
              )}
            </form.Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton variant="destructive" pending={isSubmitting} pendingLabel={t('rejecting')}>
                  {t('reject')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function WithdrawButton({ project, review }: { project: Project; review: ApprovalReview }) {
  const t = useTranslations('approvals');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [open, setOpen] = useState(false);

  async function onWithdraw() {
    const result = await withdraw(project.id, review.id);
    changed();
    if (result.ok) toast.success(t('withdrawn'));
    else toast.error(errors(result.error.code));
  }

  return (
    <>
      <Button variant="ghost" onClick={() => setOpen(true)}>
        <Undo2 aria-hidden />
        {t('withdraw')}
      </Button>
      <AlertDialog open={open} onOpenChange={setOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('withdrawTitle')}</AlertDialogTitle>
            <AlertDialogDescription>{t('withdrawConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void onWithdraw()}>{t('withdraw')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
