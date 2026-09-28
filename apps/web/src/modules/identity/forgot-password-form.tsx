'use client';

import { useForm } from '@tanstack/react-form';
import { MailCheck } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { TextField } from '@/components/shared/form/text-field';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Link } from '@/i18n/navigation';

import { requestPasswordReset } from './identity.service';
import { forgotPasswordSchema, type ForgotPasswordValues } from './schema';

export function ForgotPasswordForm() {
  const t = useTranslations('forgotPassword');
  const [error, setError] = useState<ActionError | null>(null);
  const [sentTo, setSentTo] = useState<string | null>(null);

  const form = useForm({
    defaultValues: { email: '' } satisfies ForgotPasswordValues,
    validators: { onBlur: forgotPasswordSchema, onSubmit: forgotPasswordSchema },
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
            <Alert variant="success" aria-live="polite">
              <MailCheck aria-hidden />
              <AlertTitle>{t('sentTitle')}</AlertTitle>
              <AlertDescription>{t('sent', { email: sentTo })}</AlertDescription>
            </Alert>
            {backToSignIn}
          </>
        ) : (
          <form
            noValidate
            className="grid gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              void form.handleSubmit();
            }}
          >
            {error ? <ErrorAlert error={error} /> : null}
            <form.Field name="email">
              {(field) => (
                <TextField field={field} label={t('email')} type="email" autoComplete="email" required autoFocus />
              )}
            </form.Field>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <Button type="submit" disabled={isSubmitting} className="w-full">
                  {isSubmitting ? t('submitting') : t('submit')}
                </Button>
              )}
            </form.Subscribe>
            {backToSignIn}
          </form>
        )}
      </CardContent>
    </Card>
  );
}
