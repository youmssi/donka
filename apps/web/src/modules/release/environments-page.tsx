'use client';

import { useForm } from '@tanstack/react-form';
import { Check, CircleAlert, Copy, KeyRound, Plus, Rocket, RotateCcw, ShieldCheck } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { Fragment, useId, useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { When } from '@/components/shared/format';
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
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group';
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemSeparator,
  ItemTitle,
} from '@/components/ui/item';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import { Skeleton } from '@/components/ui/skeleton';
import { Spinner } from '@/components/ui/spinner';
import { Link, useRouter } from '@/i18n/navigation';
import { approvalHref, ProjectFrame, type Project } from '@/modules/project';

import { canRelease } from './releases-page';
import { deployRelease, issueToken, requestApproval, retryDeployment, revokeToken } from './release.service';
import {
  inFlight,
  TOKEN_NAME_MAX,
  tokenSchema,
  type Deployment,
  type EnvironmentName,
  type EnvironmentState,
  type IssuedToken,
  type RuntimeToken,
  type TokenValues,
} from './schema';
import { useApprovals, useEnvironments, useReleaseList, useReleasesChanged, useTokens } from './useReleases';

/** Owners manage the tokens that open an environment. */
function canManageTokens(project: Project): boolean {
  return project.role === 'owner' && !project.archivedAt;
}

export function EnvironmentsPage() {
  return <ProjectFrame section="environments">{(project) => <Environments project={project} />}</ProjectFrame>;
}

function Environments({ project }: { project: Project }) {
  const t = useTranslations('environments');
  const common = useTranslations('common');
  const query = useEnvironments(project.id);
  const result = query.data;

  if (result && !result.ok) {
    return (
      <ErrorAlert
        error={result.error}
        title={t('errorTitle')}
        action={
          <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
            {common('retry')}
          </Button>
        }
      />
    );
  }
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      {result
        ? result.data.map((env) => (
            <EnvironmentCard
              key={env.environment}
              project={project}
              state={env}
              staging={result.data.find((other) => other.environment === 'staging')?.live ?? null}
            />
          ))
        : [0, 1].map((index) => <Skeleton key={index} className="h-80 w-full rounded-xl" />)}
    </div>
  );
}

function EnvironmentCard({
  project,
  state,
  staging,
}: {
  project: Project;
  state: EnvironmentState;
  /** What staging runs: the release production can be asked for. */
  staging: Deployment | null;
}) {
  const t = useTranslations('environments');
  const env = state.environment;
  const pending = state.latest && state.latest.id !== state.live?.id ? state.latest : null;

  return (
    <Card role="region" aria-labelledby={`env-${env}`}>
      <CardHeader>
        <CardTitle id={`env-${env}`}>{t(env)}</CardTitle>
        <CardDescription>{t(`${env}Description`)}</CardDescription>
        <CardAction>
          <Badge variant={env === 'production' ? 'default' : 'secondary'}>{`${env}/${project.key}`}</Badge>
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-4">
        <section className="grid gap-1" aria-label={t('live')}>
          <h3 className="text-xs font-medium text-muted-foreground uppercase">{t('live')}</h3>
          {state.live ? (
            <p className="flex flex-wrap items-baseline gap-x-2 text-sm">
              <span className="font-mono text-lg font-semibold">{state.live.releaseVersion}</span>
              <span className="text-muted-foreground">
                {t('publishedBy', { email: state.live.requestedBy.email })} ·{' '}
                <When value={state.live.publishedAt ?? state.live.requestedAt} as="ago" />
              </span>
            </p>
          ) : (
            <p className="text-sm text-muted-foreground">{t('nothingLive')}</p>
          )}
        </section>
        {pending ? <PendingDeployment project={project} deployment={pending} /> : null}
        {env === 'staging' ? (
          canRelease(project) ? (
            <DeployControl project={project} live={state.live?.releaseId ?? null} />
          ) : null
        ) : (
          <ProductionRequest project={project} staging={staging} live={state.live?.releaseId ?? null} />
        )}
        <Separator />
        <Tokens project={project} environment={env} />
      </CardContent>
    </Card>
  );
}

/**
 * Production is published through an approval: the request waiting for one,
 * or a way to ask for the release live on staging.
 */
function ProductionRequest({
  project,
  staging,
  live,
}: {
  project: Project;
  staging: Deployment | null;
  live: string | null;
}) {
  const t = useTranslations('environments');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const router = useRouter();
  const changed = useReleasesChanged(project.id);
  const approvals = useApprovals(project.id, 0).data;
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const pending = approvals?.ok ? approvals.data.items.find((approval) => approval.status === 'pending') : undefined;

  if (pending) {
    return (
      <Alert className="border-amber-500/40 bg-amber-500/10">
        <ShieldCheck className="text-amber-600" aria-hidden />
        <AlertDescription className="text-foreground">
          <p>{t('waitingApproval', { version: pending.releaseVersion, email: pending.requestedBy.email })}</p>
          <Button asChild size="sm" variant="outline" className="mt-2">
            <Link href={approvalHref(project.key, pending.id)}>{t('review')}</Link>
          </Button>
        </AlertDescription>
      </Alert>
    );
  }
  const askable = canRelease(project) && staging && staging.releaseId !== live;
  if (!askable) {
    return (
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <ShieldCheck className="size-4 shrink-0" aria-hidden />
        {t('productionApproval')}
      </p>
    );
  }

  async function onRequest() {
    if (!staging) return;
    setBusy(true);
    const result = await requestApproval(project.id, staging.releaseId);
    setBusy(false);
    changed();
    if (result.ok) {
      toast.success(t('requested', { version: result.data.releaseVersion }));
      router.push(approvalHref(project.key, result.data.id));
    } else {
      toast.error(errors(result.error.code));
    }
  }

  return (
    <div className="grid gap-2">
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <ShieldCheck className="size-4 shrink-0" aria-hidden />
        {t('productionApproval')}
      </p>
      <div>
        <Button disabled={busy} onClick={() => setConfirming(true)}>
          {busy ? <Spinner /> : <Rocket aria-hidden />}
          {t('requestProduction', { version: staging.releaseVersion })}
        </Button>
      </div>
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('requestTitle', { version: staging.releaseVersion })}</AlertDialogTitle>
            <AlertDialogDescription>{t('requestConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void onRequest()}>{t('requestAction')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

/** A deployment on its way, retrying, or given up: said plainly, with what to do. */
function PendingDeployment({ project, deployment }: { project: Project; deployment: Deployment }) {
  const t = useTranslations('environments');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [retrying, setRetrying] = useState(false);

  async function onRetry() {
    setRetrying(true);
    const result = await retryDeployment(project.id, deployment.environment, deployment.id);
    setRetrying(false);
    changed();
    if (!result.ok) toast.error(errors(result.error.code));
  }

  const what =
    deployment.reason === 'tokens'
      ? t('republishing', { version: deployment.releaseVersion })
      : t('deployingVersion', { version: deployment.releaseVersion });

  if (deployment.status === 'failed') {
    return (
      <Alert variant="destructive">
        <CircleAlert aria-hidden />
        <AlertTitle>{t('failedTitle', { version: deployment.releaseVersion })}</AlertTitle>
        <AlertDescription>
          <p>{t('failed', { attempts: deployment.attempts, error: deployment.lastError ?? '' })}</p>
          {canRelease(project) ? (
            <Button variant="outline" size="sm" className="mt-2" disabled={retrying} onClick={() => void onRetry()}>
              {retrying ? <Spinner /> : <RotateCcw aria-hidden />}
              {t('retry')}
            </Button>
          ) : null}
        </AlertDescription>
      </Alert>
    );
  }
  if (inFlight(deployment)) {
    return (
      <Alert role="status" aria-live="polite">
        <Spinner />
        <AlertTitle>{what}</AlertTitle>
        {deployment.status === 'retrying' ? (
          <AlertDescription>
            {t.rich('retrying', {
              attempts: deployment.attempts,
              error: deployment.lastError ?? '',
              when: () => (deployment.nextAttemptAt ? <When value={deployment.nextAttemptAt} as="time" /> : null),
            })}
          </AlertDescription>
        ) : null}
      </Alert>
    );
  }
  return null;
}

function DeployControl({ project, live }: { project: Project; live: string | null }) {
  const t = useTranslations('environments');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const releases = useReleaseList(project.id, 0).data;
  const items = releases?.ok ? releases.data.items : [];
  const [chosen, setChosen] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const selectId = useId();
  const selected = chosen ?? items[0]?.id ?? '';

  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('noRelease')}</p>;
  }

  async function onDeploy() {
    const release = items.find((r) => r.id === selected);
    if (!release) return;
    setBusy(true);
    const result = await deployRelease(project.id, 'staging', release.id);
    setBusy(false);
    changed();
    if (result.ok) toast.success(t('deployingVersion', { version: release.version }));
    else toast.error(errors(result.error.code));
  }

  return (
    <div className="flex flex-wrap items-end gap-2">
      <Field className="min-w-0 flex-1 gap-1.5">
        <FieldLabel htmlFor={selectId}>{t('deployLabel')}</FieldLabel>
        <Select value={selected} onValueChange={setChosen}>
          <SelectTrigger id={selectId} className="w-full overflow-hidden">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {items.map((release) => (
              <SelectItem key={release.id} value={release.id}>
                <span className="font-mono">{release.version}</span>
                <span className="max-w-60 min-w-0 truncate text-muted-foreground">{release.notes}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Button disabled={busy || selected === live} onClick={() => void onDeploy()}>
        {busy ? <Spinner /> : <Rocket aria-hidden />}
        {t('deploy')}
      </Button>
    </div>
  );
}

function Tokens({ project, environment }: { project: Project; environment: EnvironmentName }) {
  const t = useTranslations('tokens');
  const query = useTokens(project.id, environment);
  const result = query.data;
  const manage = canManageTokens(project);
  const [issuing, setIssuing] = useState(false);

  return (
    <section className="grid gap-2" aria-label={t('title')}>
      <div className="flex items-center gap-2">
        <h3 className="flex-1 text-xs font-medium text-muted-foreground uppercase">{t('title')}</h3>
        {manage ? (
          <Button size="sm" variant="outline" onClick={() => setIssuing(true)}>
            <Plus aria-hidden />
            {t('create')}
          </Button>
        ) : null}
      </div>
      {!result ? (
        <Skeleton className="h-16 w-full" />
      ) : !result.ok ? (
        <ErrorAlert error={result.error} />
      ) : result.data.length === 0 ? (
        <p className="text-sm text-muted-foreground">{manage ? t('empty') : t('emptyReadOnly')}</p>
      ) : (
        <ItemGroup className="rounded-lg border">
          {result.data.map((token, index) => (
            <Fragment key={token.id}>
              {index > 0 ? <ItemSeparator /> : null}
              <TokenRow project={project} environment={environment} token={token} manage={manage} />
            </Fragment>
          ))}
        </ItemGroup>
      )}
      {issuing ? (
        <IssueTokenDialog project={project} environment={environment} onClose={() => setIssuing(false)} />
      ) : null}
    </section>
  );
}

function TokenRow({
  project,
  environment,
  token,
  manage,
}: {
  project: Project;
  environment: EnvironmentName;
  token: RuntimeToken;
  manage: boolean;
}) {
  const t = useTranslations('tokens');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [confirming, setConfirming] = useState(false);
  const revoked = token.revokedAt !== null && token.revokedAt !== undefined;

  async function onRevoke() {
    const result = await revokeToken(project.id, environment, token.id);
    changed();
    if (result.ok) toast.success(t('revoked', { name: token.name }));
    else toast.error(errors(result.error.code));
  }

  return (
    <Item size="sm" role="listitem">
      <ItemMedia variant="icon">
        <KeyRound aria-hidden />
      </ItemMedia>
      <ItemContent className="min-w-0">
        <ItemTitle className={revoked ? 'text-muted-foreground line-through' : undefined}>{token.name}</ItemTitle>
        <ItemDescription className="flex flex-wrap items-center gap-x-2">
          <span className="font-mono">…{token.hint}</span>
          {revoked ? null : <When value={token.createdAt} as="ago" />}
        </ItemDescription>
      </ItemContent>
      {revoked ? <Badge variant="outline">{t('revokedBadge')}</Badge> : null}
      {manage && !revoked ? (
        <ItemActions>
          <Button variant="ghost" size="sm" onClick={() => setConfirming(true)}>
            {t('revoke')}
          </Button>
          <AlertDialog open={confirming} onOpenChange={setConfirming}>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{t('revokeTitle', { name: token.name })}</AlertDialogTitle>
                <AlertDialogDescription>{t('revokeConfirm')}</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
                <AlertDialogAction variant="destructive" onClick={() => void onRevoke()}>
                  {t('revoke')}
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </ItemActions>
      ) : null}
    </Item>
  );
}

/** Names a new token, then shows it once, with a way to copy it. */
function IssueTokenDialog({
  project,
  environment,
  onClose,
}: {
  project: Project;
  environment: EnvironmentName;
  onClose: () => void;
}) {
  const t = useTranslations('tokens');
  const environments = useTranslations('environments');
  const common = useTranslations('common');
  const changed = useReleasesChanged(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const [issued, setIssued] = useState<IssuedToken | null>(null);
  const [copied, setCopied] = useState(false);
  const tokenField = useId();

  const form = useForm({
    defaultValues: { name: '' } satisfies TokenValues,
    validators: { onChange: tokenSchema, onSubmit: tokenSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await issueToken(project.id, environment, value.name.trim());
      if (result.ok) {
        changed();
        setIssued(result.data);
      } else {
        setError(result.error);
      }
    },
  });

  async function copy(token: string) {
    try {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      toast.success(t('copied'));
    } catch {
      toast.error(t('copyFailed'));
    }
  }

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('createTitle', { environment: environments(environment) })}</DialogTitle>
          <DialogDescription>{issued ? t('shownOnce') : t('createDescription')}</DialogDescription>
        </DialogHeader>
        {issued ? (
          <div className="grid gap-4">
            <Field className="gap-1.5">
              <FieldLabel htmlFor={tokenField}>{issued.name}</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id={tokenField}
                  readOnly
                  value={issued.token}
                  className="font-mono text-xs"
                  onFocus={(e) => e.target.select()}
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupButton
                    size="icon-xs"
                    aria-label={t('copy')}
                    title={t('copy')}
                    onClick={() => void copy(issued.token)}
                  >
                    {copied ? <Check aria-hidden /> : <Copy aria-hidden />}
                  </InputGroupButton>
                </InputGroupAddon>
              </InputGroup>
            </Field>
            <Alert>
              <CircleAlert aria-hidden />
              <AlertDescription>{t('header')}</AlertDescription>
            </Alert>
            <DialogFooter>
              <Button onClick={onClose}>{t('done')}</Button>
            </DialogFooter>
          </div>
        ) : (
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
              <form.Field name="name">
                {(field) => (
                  <TextField
                    field={field}
                    label={t('name')}
                    hint={t('nameHint')}
                    placeholder={t('namePlaceholder')}
                    required
                    autoFocus
                    messageValues={{ max: TOKEN_NAME_MAX }}
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
                  <SubmitButton pending={isSubmitting} pendingLabel={t('creating')}>
                    {t('create')}
                  </SubmitButton>
                )}
              </form.Subscribe>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
