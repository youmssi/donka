'use client';

import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';

import { ORGANISATION_MAX, organisationSchema, type OrganisationValues } from './schema';
import { useInstallation, useRenameOrganisation } from './useInstallation';

/** The organisation's name, shown in Studio's header, its emails and exports (administrators). */
export function OrganisationCard() {
  const t = useTranslations('installation');
  const result = useInstallation().data;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('organisationTitle')}</CardTitle>
        <CardDescription>{t('organisationDescription')}</CardDescription>
      </CardHeader>
      <CardContent>
        {!result ? <Skeleton className="h-20 w-full" /> : null}
        {result && !result.ok ? <ErrorAlert error={result.error} /> : null}
        {result?.ok && result.data.firstRun ? (
          <p className="text-sm text-muted-foreground">{t('nameInFirstRun')}</p>
        ) : null}
        {result?.ok && !result.data.firstRun ? <RenameForm current={result.data.organisation ?? ''} /> : null}
      </CardContent>
    </Card>
  );
}

function RenameForm({ current }: { current: string }) {
  const t = useTranslations('installation');
  const common = useTranslations('common');
  const rename = useRenameOrganisation();
  const [error, setError] = useState<ActionError | null>(null);
  const form = useAppForm({
    defaultValues: { organisation: current } satisfies OrganisationValues,
    validators: { onChange: organisationSchema, onSubmit: organisationSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await rename.mutateAsync(organisationSchema.parse(value));
      if (result.ok) toast.success(t('renamed'));
      else setError(result.error);
    },
  });

  return (
    <form.AppForm>
      <form.Form className="grid gap-4 sm:max-w-md">
        {error ? <ErrorAlert error={error} /> : null}
        <form.AppField name="organisation">
          {(field) => (
            <field.TextField
              label={t('organisationName')}
              autoComplete="organization"
              required
              messageValues={{ max: ORGANISATION_MAX }}
            />
          )}
        </form.AppField>
        <form.SubmitButton pendingLabel={common('saving')} className="justify-self-start">
          {common('save')}
        </form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}
