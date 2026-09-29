'use client';

import { useForm } from '@tanstack/react-form';
import { CircleCheck } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useEffect, useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Link, useRouter } from '@/i18n/navigation';

import { safeNext } from './safe-next';
import { signInSchema, type SignInValues } from './schema';
import { useSession, useSignIn } from './useSession';

export function SignInForm() {
  const t = useTranslations('signIn');
  const router = useRouter();
  const searchParams = useSearchParams();
  const next = safeNext(searchParams.get('next'));
  const session = useSession();
  const signIn = useSignIn();
  const [error, setError] = useState<ActionError | null>(null);

  const signedIn = session.data?.ok === true && session.data.data !== null;
  useEffect(() => {
    if (signedIn) router.replace(next);
  }, [signedIn, next, router]);

  const form = useForm({
    defaultValues: { email: '', password: '' } satisfies SignInValues,
    validators: { onChange: signInSchema, onSubmit: signInSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await signIn.mutateAsync(value);
      if (!result.ok) setError(result.error);
    },
  });

  if (session.isPending || signedIn) return <PageSkeleton />;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description')}</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {searchParams.get('signedOut') && !error ? (
            <Alert>
              <CircleCheck aria-hidden className="text-success" />
              <AlertDescription>{t('signedOut')}</AlertDescription>
            </Alert>
          ) : null}
          {error ? <ErrorAlert error={error} /> : null}
          <form.Field name="email">
            {(field) => (
              <TextField field={field} label={t('email')} type="email" autoComplete="username" required autoFocus />
            )}
          </form.Field>
          <form.Field name="password">
            {(field) => (
              <TextField field={field} label={t('password')} type="password" autoComplete="current-password" required />
            )}
          </form.Field>
          <form.Subscribe selector={(state) => state.isSubmitting}>
            {(isSubmitting) => (
              <SubmitButton pending={isSubmitting} pendingLabel={t('submitting')} className="w-full">
                {t('submit')}
              </SubmitButton>
            )}
          </form.Subscribe>
          <Link
            href="/forgot-password"
            className="justify-self-center text-sm text-primary underline-offset-4 hover:underline"
          >
            {t('forgotPassword')}
          </Link>
        </form>
      </CardContent>
    </Card>
  );
}
