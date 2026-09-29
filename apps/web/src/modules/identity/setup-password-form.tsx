'use client';

import { useForm } from '@tanstack/react-form';
import { CircleCheck } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Link } from '@/i18n/navigation';

import { setPassword } from './identity.service';
import { PASSWORD_MAX, PASSWORD_MIN, setupPasswordSchema, type SetupPasswordValues } from './schema';

const limits = { min: PASSWORD_MIN, max: PASSWORD_MAX };

/** Opened from an invitation, password-reset or first-administrator link (`?token=`). */
export function SetupPasswordForm() {
  const t = useTranslations('setupPassword');
  const token = useSearchParams().get('token');
  const [error, setError] = useState<ActionError | null>(null);
  const [done, setDone] = useState(false);

  const form = useForm({
    defaultValues: { password: '', confirm: '' } satisfies SetupPasswordValues,
    validators: { onChange: setupPasswordSchema, onSubmit: setupPasswordSchema },
    onSubmit: async ({ value }) => {
      if (!token) return;
      setError(null);
      const result = await setPassword(token, value.password);
      if (result.ok) setDone(true);
      else setError(result.error);
    },
  });

  const requestNewLink = (
    <Link href="/forgot-password" className="text-sm font-medium text-primary underline-offset-4 hover:underline">
      {t('requestNewLink')}
    </Link>
  );

  if (done) {
    return (
      <Card>
        <CardContent className="grid gap-4">
          <Alert aria-live="polite">
            <CircleCheck aria-hidden className="text-success" />
            <AlertTitle>{t('doneTitle')}</AlertTitle>
            <AlertDescription>{t('done')}</AlertDescription>
          </Alert>
          <Button asChild className="w-full">
            <Link href="/sign-in">{t('signIn')}</Link>
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description', limits)}</CardDescription>
      </CardHeader>
      <CardContent>
        {token ? (
          <form
            noValidate
            className="grid gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              void form.handleSubmit();
            }}
          >
            {error ? (
              <ErrorAlert error={error} action={error.code === 'INVALID_SETUP_LINK' ? requestNewLink : undefined} />
            ) : null}
            <form.Field name="password">
              {(field) => (
                <TextField
                  field={field}
                  label={t('password')}
                  type="password"
                  autoComplete="new-password"
                  required
                  autoFocus
                  messageValues={limits}
                />
              )}
            </form.Field>
            <form.Field name="confirm">
              {(field) => (
                <TextField field={field} label={t('confirm')} type="password" autoComplete="new-password" required />
              )}
            </form.Field>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton pending={isSubmitting} pendingLabel={t('submitting')} className="w-full">
                  {t('submit')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </form>
        ) : (
          <div className="grid gap-4">
            <ErrorAlert error={{ code: 'INVALID_SETUP_LINK' }} />
            <p className="text-sm text-muted-foreground">{t('missingToken')}</p>
            {requestNewLink}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
