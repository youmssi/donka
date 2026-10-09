'use client';

import { MailCheck } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Link } from '@/i18n/navigation';

import { requestPasswordReset } from './identity.service';
import { forgotPasswordSchema, type ForgotPasswordValues } from './schema';

export function ForgotPasswordForm() {
  const t = useTranslations('forgotPassword');
  const [error, setError] = useState<ActionError | null>(null);
  const [sentTo, setSentTo] = useState<string | null>(null);

  const form = useAppForm({
    defaultValues: { email: '' } satisfies ForgotPasswordValues,
    validators: { onChange: forgotPasswordSchema, onSubmit: forgotPasswordSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await requestPasswordReset(value);
      if (result.ok) setSentTo(value.email.trim());
      else setError(result.error);
    },
  });

  const backToSignIn = (
    <Link href="/sign-in" className="justify-self-center text-sm text-primary underline-offset-4 hover:underline">
      {t('backToSignIn')}
    </Link>
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        {sentTo ? null : <CardDescription>{t('description')}</CardDescription>}
      </CardHeader>
      <CardContent className="grid gap-4">
        {sentTo ? (
          <>
            {/* The same answer for every address: the page must not reveal who has an account. */}
            <Alert aria-live="polite">
              <MailCheck aria-hidden className="text-success" />
              <AlertTitle>{t('sentTitle')}</AlertTitle>
              <AlertDescription>{t('sent', { email: sentTo })}</AlertDescription>
            </Alert>
            {backToSignIn}
          </>
        ) : (
          <form.AppForm>
            <form.Form className="grid gap-4">
              {error ? <ErrorAlert error={error} /> : null}
              <form.AppField name="email">
                {(field) => <field.TextField label={t('email')} type="email" autoComplete="email" required autoFocus />}
              </form.AppField>
              <form.SubmitButton pendingLabel={t('submitting')} className="w-full">
                {t('submit')}
              </form.SubmitButton>
              {backToSignIn}
            </form.Form>
          </form.AppForm>
        )}
      </CardContent>
    </Card>
  );
}
