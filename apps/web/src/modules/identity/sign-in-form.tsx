'use client';

import { CircleCheck } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useEffect, useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
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

  const form = useAppForm({
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
        <form.AppForm>
          <form.Form className="grid gap-4">
            {searchParams.get('signedOut') && !error ? (
              <Alert>
                <CircleCheck aria-hidden className="text-success" />
                <AlertDescription>{t('signedOut')}</AlertDescription>
              </Alert>
            ) : null}
            {error ? <ErrorAlert error={error} /> : null}
            <form.AppField name="email">
              {(field) => (
                <field.TextField label={t('email')} type="email" autoComplete="username" required autoFocus />
              )}
            </form.AppField>
            <form.AppField name="password">
              {(field) => (
                <field.TextField label={t('password')} type="password" autoComplete="current-password" required />
              )}
            </form.AppField>
            <form.SubmitButton pendingLabel={t('submitting')} className="w-full">
              {t('submit')}
            </form.SubmitButton>
            <Link
              href="/forgot-password"
              className="justify-self-center text-sm text-primary underline-offset-4 hover:underline"
            >
              {t('forgotPassword')}
            </Link>
          </form.Form>
        </form.AppForm>
      </CardContent>
    </Card>
  );
}
