'use client';

import { useForm } from '@tanstack/react-form';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { FieldGroup } from '@/components/ui/field';
import { Skeleton } from '@/components/ui/skeleton';
import type { Project } from '@/modules/project';

import { OUTCOME_FIELD_MAX, settingsSchema, type LogSettings } from './schema';
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
          <OutcomeForm key={result.data.outcomeField ?? ''} project={project} settings={result.data} />
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

  const form = useForm({
    defaultValues: { outcomeField: settings.outcomeField ?? '' },
    validators: { onChange: settingsSchema, onSubmit: settingsSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const outcomeField = value.outcomeField.trim();
      const result = await save.mutateAsync({ outcomeField: outcomeField || null });
      if (result.ok) toast.success(t('settingsSaved'));
      else setError(result.error);
    },
  });

  return (
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
        <form.Field name="outcomeField">
          {(field) => (
            <TextField
              field={field}
              label={t('outcomeField')}
              hint={editable ? t('outcomeFieldHint') : t('outcomeFieldReadOnly')}
              placeholder="decision"
              disabled={!editable}
              messageValues={{ max: OUTCOME_FIELD_MAX }}
            />
          )}
        </form.Field>
      </FieldGroup>
      {editable ? (
        <form.Subscribe selector={(state) => state.isSubmitting}>
          {(isSubmitting) => (
            <SubmitButton pending={isSubmitting} pendingLabel={common('saving')} className="w-fit">
              {common('save')}
            </SubmitButton>
          )}
        </form.Subscribe>
      ) : null}
    </form>
  );
}
