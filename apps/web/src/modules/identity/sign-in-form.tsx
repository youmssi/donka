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
import { signInSchema, type SignInValues, type User } from './schema';
import { CodeForm, EnrolmentSteps, RecoveryCodes } from './second-step';
import {
  useCompleteSignIn,
  useConfirmSignInEnrolment,
  useFinishSignIn,
  useSession,
  useSignIn,
  useStartSignInEnrolment,
} from './useSession';

/** Where a sign-in stands: the password, then a second step when the account has or needs one. */
type Step =
  { kind: 'password' } | { kind: 'code' } | { kind: 'enrol' } | { kind: 'codes'; user: User; codes: string[] };

export function SignInForm() {
  const t = useTranslations('signIn');
  const router = useRouter();
  const searchParams = useSearchParams();
  const next = safeNext(searchParams.get('next'));
  const session = useSession();
  const signIn = useSignIn();
  const [error, setError] = useState<ActionError | null>(null);
  const [step, setStep] = useState<Step>({ kind: 'password' });

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
      else if (result.data.step !== 'signedIn') setStep({ kind: result.data.step });
    },
  });

  /** The password step expired or was refused: start over with a message. */
  function restart(expired: ActionError) {
    setStep({ kind: 'password' });
    form.setFieldValue('password', '');
    setError(expired);
  }

  if (session.isPending || signedIn) return <PageSkeleton />;
  if (step.kind !== 'password') return <SecondStepCard step={step} onStep={setStep} onExpired={restart} />;

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

function SecondStepCard({
  step,
  onStep,
  onExpired,
}: {
  step: Exclude<Step, { kind: 'password' }>;
  onStep: (step: Step) => void;
  onExpired: (error: ActionError) => void;
}) {
  const t = useTranslations('secondStep');
  const complete = useCompleteSignIn();
  const startEnrolment = useStartSignInEnrolment();
  const confirmEnrolment = useConfirmSignInEnrolment();
  const finish = useFinishSignIn();

  const title = { code: t('signInTitle'), enrol: t('enrolTitle'), codes: t('codesTitle') }[step.kind];
  const description = {
    code: t('signInDescription'),
    enrol: t('enrolRequiredDescription'),
    codes: t('codesDescription'),
  }[step.kind];

  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {step.kind === 'code' ? (
          <CodeForm
            recovery
            submitLabel={t('verify')}
            submit={async (values) => {
              const result = await complete.mutateAsync(values);
              if (result.ok) return null;
              if (result.error.code === 'SIGN_IN_EXPIRED') onExpired(result.error);
              // The server answers a wrong code like a wrong password (no hint about a
              // lockout); at this step the person only typed a code, so say that.
              return result.error.code === 'INVALID_CREDENTIALS'
                ? { ...result.error, code: 'INVALID_CODE' }
                : result.error;
            }}
          />
        ) : null}
        {step.kind === 'enrol' ? (
          <EnrolmentSteps
            start={async () => {
              const result = await startEnrolment.mutateAsync();
              if (!result.ok && result.error.code === 'SIGN_IN_EXPIRED') onExpired(result.error);
              return result;
            }}
            confirm={async (values) => {
              const result = await confirmEnrolment.mutateAsync(values);
              if (!result.ok) {
                if (result.error.code === 'SIGN_IN_EXPIRED') onExpired(result.error);
                return result;
              }
              onStep({ kind: 'codes', user: result.data.user, codes: result.data.recoveryCodes });
              return { ok: true, data: result.data.recoveryCodes };
            }}
            onConfirmed={() => undefined}
          />
        ) : null}
        {step.kind === 'codes' ? (
          <RecoveryCodes codes={step.codes} doneLabel={t('continue')} onDone={() => finish(step.user)} />
        ) : null}
      </CardContent>
    </Card>
  );
}
