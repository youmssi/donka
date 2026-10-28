'use client';

import { Scale } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { useDateFormat } from '@/components/shared/format';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { FieldGroup } from '@/components/ui/field';
import { Skeleton } from '@/components/ui/skeleton';
import type { Project } from '@/modules/project';

import {
  dayDate,
  LEGAL_REFERENCE_MAX,
  localToday,
  RULE_DESCRIPTION_MAX,
  ruleSchema,
  type RetentionRule,
  type RuleValues,
} from './schema';
import { useCreateRetentionRule, useEndRetentionRule, useRetentionRules } from './useDecisionLog';

/** The rules that require keeping the project's records whole; while one runs, nobody's records are anonymised. */
export function RetentionRules({ project }: { project: Project }) {
  const t = useTranslations('retentionRules');
  const query = useRetentionRules(project.id);
  const result = query.data;
  const owner = project.role === 'owner';
  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description')}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6">
        {!result ? (
          <Skeleton className="h-16 w-full" />
        ) : !result.ok ? (
          <ErrorAlert error={result.error} />
        ) : result.data.length === 0 ? (
          <Empty className="border border-dashed">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Scale />
              </EmptyMedia>
              <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
              <EmptyDescription>{t('empty')}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <ul className="grid gap-3" aria-label={t('title')}>
            {result.data.map((rule) => (
              <Rule key={rule.id} project={project} rule={rule} />
            ))}
          </ul>
        )}
        {owner ? <NewRule project={project} /> : <p className="text-sm text-muted-foreground">{t('readOnly')}</p>}
      </CardContent>
    </Card>
  );
}

function Rule({ project, rule }: { project: Project; rule: RetentionRule }) {
  const t = useTranslations('retentionRules');
  const format = useDateFormat();
  const until = format.date(dayDate(rule.until));
  return (
    <li className="flex flex-wrap items-start justify-between gap-3 rounded-lg border p-3">
      <div className="grid min-w-0 gap-1">
        <div className="flex flex-wrap items-center gap-2">
          {rule.running ? (
            <Badge>{t('running')}</Badge>
          ) : (
            <Badge variant="outline">{rule.endedAt ? t('ended') : t('expired')}</Badge>
          )}
          <span className="text-sm text-muted-foreground">{t('until', { date: until })}</span>
        </div>
        <p className="text-sm break-words">{rule.description}</p>
        {rule.legalReference ? (
          <p className="text-xs break-words text-muted-foreground">{rule.legalReference}</p>
        ) : null}
      </div>
      {rule.running && project.role === 'owner' ? <EndRule project={project} rule={rule} /> : null}
    </li>
  );
}

function EndRule({ project, rule }: { project: Project; rule: RetentionRule }) {
  const t = useTranslations('retentionRules');
  const common = useTranslations('common');
  const end = useEndRetentionRule(project.id);
  const [error, setError] = useState<ActionError | null>(null);

  async function run() {
    setError(null);
    const result = await end.mutateAsync(rule.id);
    if (result.ok) toast.success(t('endedToast'));
    else setError(result.error);
  }

  return (
    <div className="grid gap-2">
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button variant="outline" size="sm">
            {t('end')}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('endTitle')}</AlertDialogTitle>
            <AlertDialogDescription>{t('endConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void run()}>{t('end')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      {error ? <ErrorAlert error={error} /> : null}
    </div>
  );
}

function NewRule({ project }: { project: Project }) {
  const t = useTranslations('retentionRules');
  const create = useCreateRetentionRule(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const today = localToday();
  const schema = ruleSchema(today);

  const defaultValues: RuleValues = { description: '', legalReference: '', until: '' };
  const form = useAppForm({
    defaultValues,
    validators: { onChange: schema, onSubmit: schema },
    onSubmit: async ({ value, formApi }) => {
      setError(null);
      const result = await create.mutateAsync({
        description: value.description.trim(),
        legalReference: value.legalReference.trim() || null,
        until: value.until,
      });
      if (!result.ok) {
        setError(result.error);
        return;
      }
      formApi.reset();
      toast.success(t('created'));
    },
  });

  return (
    <form.AppForm>
      <form.Form className="grid gap-4 border-t pt-6">
        <p className="text-sm font-medium">{t('newTitle')}</p>
        {error ? <ErrorAlert error={error} /> : null}
        <FieldGroup className="gap-2">
          <form.AppField name="description">
            {(field) => (
              <field.TextField
                label={t('ruleDescription')}
                hint={t('ruleDescriptionHint')}
                multiline
                rows={2}
                required
                messageValues={{ max: RULE_DESCRIPTION_MAX }}
              />
            )}
          </form.AppField>
          <div className="grid gap-2 sm:grid-cols-2">
            <form.AppField name="legalReference">
              {(field) => (
                <field.TextField
                  label={t('legalReference')}
                  hint={t('legalReferenceHint')}
                  messageValues={{ max: LEGAL_REFERENCE_MAX }}
                />
              )}
            </form.AppField>
            <form.AppField name="until">
              {(field) => (
                <field.TextField label={t('untilLabel')} hint={t('untilHint')} type="date" min={today} required />
              )}
            </form.AppField>
          </div>
        </FieldGroup>
        <form.SubmitButton pendingLabel={t('creating')} className="w-fit">
          {t('create')}
        </form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}
