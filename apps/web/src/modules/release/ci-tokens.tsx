'use client';

import { useForm } from '@tanstack/react-form';
import { KeyRound, Plus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { Fragment, useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { When } from '@/components/shared/format';
import { ShownOnceToken } from '@/components/shared/shown-once-token';
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
import { FieldGroup } from '@/components/ui/field';
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
import { Skeleton } from '@/components/ui/skeleton';
import type { Project } from '@/modules/project';

import { issueCiToken, revokeCiToken } from './release.service';
import { TOKEN_NAME_MAX, tokenSchema, type CiToken, type IssuedCiToken, type TokenValues } from './schema';
import { useCiTokens, useReleasesChanged } from './useReleases';

/** The tokens CI pipelines pull this project's artifacts with; owners issue and revoke them. */
export function CiTokens({ project }: { project: Project }) {
  const t = useTranslations('ciTokens');
  const query = useCiTokens(project.id);
  const result = query.data;
  const manage = project.role === 'owner' && !project.archivedAt;
  const [issuing, setIssuing] = useState(false);

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description')}</CardDescription>
        {manage ? (
          <CardAction>
            <Button size="sm" variant="outline" onClick={() => setIssuing(true)}>
              <Plus aria-hidden />
              {t('create')}
            </Button>
          </CardAction>
        ) : null}
      </CardHeader>
      <CardContent>
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
                <TokenRow project={project} token={token} manage={manage} />
              </Fragment>
            ))}
          </ItemGroup>
        )}
      </CardContent>
      {issuing ? <IssueDialog project={project} onClose={() => setIssuing(false)} /> : null}
    </Card>
  );
}

function TokenRow({ project, token, manage }: { project: Project; token: CiToken; manage: boolean }) {
  const t = useTranslations('ciTokens');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [confirming, setConfirming] = useState(false);
  const revoked = Boolean(token.revokedAt);

  async function onRevoke() {
    const result = await revokeCiToken(project.id, token.id);
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
          {revoked ? null : token.lastUsedAt ? (
            <span>
              {t('lastUsed')} <When value={token.lastUsedAt} as="ago" />
            </span>
          ) : (
            <span>{t('neverUsed')}</span>
          )}
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

/** Names a token, then shows it once with what a pipeline needs to use it. */
function IssueDialog({ project, onClose }: { project: Project; onClose: () => void }) {
  const t = useTranslations('ciTokens');
  const common = useTranslations('common');
  const changed = useReleasesChanged(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const [issued, setIssued] = useState<IssuedCiToken | null>(null);

  const form = useForm({
    defaultValues: { name: '' } satisfies TokenValues,
    validators: { onChange: tokenSchema, onSubmit: tokenSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await issueCiToken(project.id, value.name.trim());
      if (result.ok) {
        changed();
        setIssued(result.data);
      } else {
        setError(result.error);
      }
    },
  });

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('createTitle')}</DialogTitle>
          <DialogDescription>{issued ? t('shownOnce') : t('createDescription')}</DialogDescription>
        </DialogHeader>
        {issued ? (
          <div className="grid gap-4">
            <ShownOnceToken label={issued.name} token={issued.token} />
            <div className="grid gap-1.5">
              <p className="text-sm font-medium">{t('pipelineSettings')}</p>
              <pre className="rounded-md bg-muted p-3 font-mono text-xs break-all whitespace-pre-wrap">
                {`DONKA_URL=${window.location.origin}\nDONKA_TOKEN=${issued.token}\nDONKA_PROJECT=${project.key}`}
              </pre>
              <p className="text-xs text-muted-foreground">{t('targets')}</p>
            </div>
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
