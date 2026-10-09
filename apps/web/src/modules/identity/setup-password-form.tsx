'use client';

import { CircleCheck } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
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

  const form = useAppForm({
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
          <form.AppForm>
            <form.Form className="grid gap-4">
              {error ? (
                <ErrorAlert error={error} action={error.code === 'INVALID_SETUP_LINK' ? requestNewLink : undefined} />
              ) : null}
              <form.AppField name="password">
                {(field) => (
                  <field.TextField
                    label={t('password')}
                    type="password"
                    autoComplete="new-password"
                    required
                    autoFocus
                    messageValues={limits}
                  />
                )}
              </form.AppField>
              <form.AppField name="confirm">
                {(field) => (
                  <field.TextField label={t('confirm')} type="password" autoComplete="new-password" required />
                )}
              </form.AppField>
              <form.SubmitButton pendingLabel={t('submitting')} className="w-full">
                {t('submit')}
              </form.SubmitButton>
            </form.Form>
          </form.AppForm>
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
