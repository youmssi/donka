'use client';

import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { FieldGroup } from '@/components/ui/field';
import { Skeleton } from '@/components/ui/skeleton';
import type { Project } from '@/modules/project';

import { fieldLines, OUTCOME_FIELD_MAX, REDACTED_FIELDS_MAX, settingsSchema, type LogSettings } from './schema';
import { useLogSettings, useSaveLogSettings } from './useDecisionLog';

/** The project's decision-log settings: which output field is a record's outcome (owners change it). */
export function OutcomeSettings({ project }: { project: Project }) {
  const t = useTranslations('decisionLog');
  const query = useLogSettings(project.id);
  const result = query.data;
  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t('settingsTitle')}</CardTitle>
        <CardDescription>{t('settingsDescription')}</CardDescription>
      </CardHeader>
      <CardContent>
        {!result ? (
          <Skeleton className="h-16 w-full" />
        ) : !result.ok ? (
          <ErrorAlert error={result.error} />
        ) : (
          <OutcomeForm
            key={`${result.data.outcomeField ?? ''}|${result.data.redactedFields.join(',')}`}
            project={project}
            settings={result.data}
          />
        )}
      </CardContent>
    </Card>
  );
}

function OutcomeForm({ project, settings }: { project: Project; settings: LogSettings }) {
  const t = useTranslations('decisionLog');
  const common = useTranslations('common');
  const editable = project.role === 'owner' && !project.archivedAt;
  const save = useSaveLogSettings(project.id);
  const [error, setError] = useState<ActionError | null>(null);

  const form = useAppForm({
    defaultValues: { outcomeField: settings.outcomeField ?? '', redactedFields: settings.redactedFields.join('\n') },
    validators: { onChange: settingsSchema, onSubmit: settingsSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const outcomeField = value.outcomeField.trim();
      const result = await save.mutateAsync({
        outcomeField: outcomeField || null,
        redactedFields: [...new Set(fieldLines(value.redactedFields))],
      });
      if (result.ok) toast.success(t('settingsSaved'));
      else setError(result.error);
    },
  });

  return (
    <form.AppForm>
      <form.Form className="grid gap-4">
        {error ? <ErrorAlert error={error} /> : null}
        <FieldGroup className="gap-2">
          <form.AppField name="outcomeField">
            {(field) => (
              <field.TextField
                label={t('outcomeField')}
                hint={editable ? t('outcomeFieldHint') : t('outcomeFieldReadOnly')}
                placeholder="decision"
                disabled={!editable}
                messageValues={{ max: OUTCOME_FIELD_MAX }}
              />
            )}
          </form.AppField>
          <form.AppField name="redactedFields">
            {(field) => (
              <field.TextField
                label={t('redactedFields')}
                hint={t(settings.explainEnabled ? 'redactedFieldsHint' : 'redactedFieldsHintOff', {
                  max: REDACTED_FIELDS_MAX,
                })}
                placeholder={'applicant.nationalId\napplicant.name'}
                multiline
                disabled={!editable}
                messageValues={{ max: REDACTED_FIELDS_MAX }}
              />
            )}
          </form.AppField>
        </FieldGroup>
        {editable ? (
          <form.SubmitButton pendingLabel={common('saving')} className="w-fit">
            {common('save')}
          </form.SubmitButton>
        ) : null}
      </form.Form>
    </form.AppForm>
  );
}
