'use client';

import { CircleAlert, Plus, RadioTower } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { When } from '@/components/shared/format';
import { PageHeader } from '@/components/shared/layout/page-header';
import { ShownOnceToken } from '@/components/shared/shown-once-token';
import { Alert, AlertDescription } from '@/components/ui/alert';
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
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { FieldGroup } from '@/components/ui/field';

import { EnvironmentBadge } from './decision-log-page';
import { issueToken, revokeToken } from './decision-log.service';
import { feedUrl, TOKEN_NAME_MAX, tokenSchema, type IssuedLogToken, type LogToken, type TokenValues } from './schema';
import { useLogTokens, useLogTokensChanged } from './useDecisionLog';

/**
 * The tokens Runtimes send their decisions to Studio with (administrators only).
 * One environment each, so a staging Runtime cannot write production records.
 */
export function RuntimesPage() {
  const t = useTranslations('runtimes');
  const common = useTranslations('common');
  const query = useLogTokens();
  const result = query.data;
  const [issuing, setIssuing] = useState(false);

  if (result && !result.ok && result.error.code === 'FORBIDDEN') {
    return <ErrorAlert error={result.error} title={t('forbiddenTitle')} />;
  }

  const columns: DataTableColumn<LogToken>[] = [
    {
      id: 'name',
      header: t('name'),
      cell: ({ row }) => <TokenName token={row.original} />,
      meta: { className: 'w-full max-w-0' },
    },
    {
      id: 'environment',
      header: t('environment'),
      cell: ({ row }) => <EnvironmentBadge environment={row.original.environment} />,
    },
    {
      id: 'issued',
      header: t('issued'),
      cell: ({ row }) => (
        <div className="grid gap-0.5">
          <When value={row.original.createdAt} />
          <span className="truncate text-xs text-muted-foreground">{row.original.createdBy.email}</span>
        </div>
      ),
      meta: { className: 'hidden w-56 md:table-cell' },
    },
    {
      id: 'actions',
      header: () => <span className="sr-only">{common('actions')}</span>,
      cell: ({ row }) =>
        row.original.revokedAt ? <Badge variant="outline">{t('revokedBadge')}</Badge> : <Revoke token={row.original} />,
      meta: { className: 'w-24 text-right' },
    },
  ];

  return (
    <div className="grid gap-4">
      <PageHeader
        title={t('title')}
        description={t('description')}
        actions={
          <Button onClick={() => setIssuing(true)}>
            <Plus aria-hidden />
            {t('create')}
          </Button>
        }
      />
      {result && !result.ok ? (
        <ErrorAlert
          error={result.error}
          action={
            <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
              {common('retry')}
            </Button>
          }
        />
      ) : (
        <DataTable
          label={t('title')}
          columns={columns}
          data={result?.data}
          getRowId={(token) => token.id}
          empty={
            <Empty className="border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <RadioTower />
                </EmptyMedia>
                <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
                <EmptyDescription>{t('empty')}</EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button size="sm" variant="outline" onClick={() => setIssuing(true)}>
                  <Plus aria-hidden />
                  {t('create')}
                </Button>
              </EmptyContent>
            </Empty>
          }
        />
      )}
      {issuing ? <IssueDialog onClose={() => setIssuing(false)} /> : null}
    </div>
  );
}

function TokenName({ token }: { token: LogToken }) {
  const revoked = Boolean(token.revokedAt);
  return (
    <div className="grid min-w-0 gap-0.5">
      <span className={revoked ? 'truncate text-muted-foreground line-through' : 'truncate'}>{token.name}</span>
      <span className="font-mono text-xs text-muted-foreground">…{token.hint}</span>
    </div>
  );
}

function Revoke({ token }: { token: LogToken }) {
  const t = useTranslations('runtimes');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useLogTokensChanged();
  const [confirming, setConfirming] = useState(false);

  async function onRevoke() {
    const result = await revokeToken(token.id);
    changed();
    if (result.ok) toast.success(t('revoked', { name: token.name }));
    else toast.error(errors(result.error.code));
  }

  return (
    <>
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
    </>
  );
}

/** Names a token for an environment, then shows it once with the Runtime's settings. */
function IssueDialog({ onClose }: { onClose: () => void }) {
  const t = useTranslations('runtimes');
  const environments = useTranslations('environments');
  const common = useTranslations('common');
  const changed = useLogTokensChanged();
  const [error, setError] = useState<ActionError | null>(null);
  const [issued, setIssued] = useState<IssuedLogToken | null>(null);

  const form = useAppForm({
    defaultValues: { environment: 'production', name: '' } as TokenValues,
    validators: { onChange: tokenSchema, onSubmit: tokenSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await issueToken({ ...value, name: value.name.trim() });
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
              <p className="text-sm font-medium">{t('runtimeSettings')}</p>
              <pre className="rounded-md bg-muted p-3 font-mono text-xs break-all whitespace-pre-wrap">
                {`DECISION_LOG__URL=${feedUrl(window.location.origin)}\nDECISION_LOG__TOKEN=${issued.token}`}
              </pre>
            </div>
            <Alert>
              <CircleAlert aria-hidden />
              <AlertDescription>
                {t('oneEnvironment', { environment: environments(issued.environment) })}
              </AlertDescription>
            </Alert>
            <DialogFooter>
              <Button onClick={onClose}>{t('done')}</Button>
            </DialogFooter>
          </div>
        ) : (
          <form.AppForm>
            <form.Form className="grid gap-4">
              {error ? <ErrorAlert error={error} /> : null}
              <FieldGroup className="gap-2">
                <form.AppField name="environment">
                  {(field) => (
                    <field.SelectField
                      label={t('environment')}
                      triggerClassName="w-fit"
                      options={[
                        { value: 'production', label: environments('production') },
                        { value: 'staging', label: environments('staging') },
                      ]}
                    />
                  )}
                </form.AppField>
                <form.AppField name="name">
                  {(field) => (
                    <field.TextField
                      label={t('name')}
                      hint={t('nameHint')}
                      placeholder={t('namePlaceholder')}
                      required
                      messageValues={{ max: TOKEN_NAME_MAX }}
                    />
                  )}
                </form.AppField>
              </FieldGroup>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={onClose}>
                  {common('cancel')}
                </Button>
                <form.SubmitButton pendingLabel={t('creating')}>{t('create')}</form.SubmitButton>
              </DialogFooter>
            </form.Form>
          </form.AppForm>
        )}
      </DialogContent>
    </Dialog>
  );
}
