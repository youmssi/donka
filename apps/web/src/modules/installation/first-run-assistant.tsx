'use client';

import { Check } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState, type ReactNode } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { cn } from '@/components/shared/utils';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { Link, useRouter } from '@/i18n/navigation';

import { ORGANISATION_MAX, organisationSchema, type OrganisationValues } from './schema';
import { useFinishFirstRun, useInstallation } from './useInstallation';

const STEPS = ['secondStep', 'organisation', 'packs'] as const;
type Step = (typeof STEPS)[number];

/**
 * The first-run assistant (DNK-74): the administrator's second sign-in step, the organisation's
 * name, the packs to start with; then the Get started checklist. It shows only during the first
 * run. The second step and the packs come from their own modules, each calling `next` when done.
 */
export function FirstRunAssistant({
  isAdmin,
  secondStep,
  packs,
}: {
  isAdmin: boolean;
  secondStep: (next: () => void, label: string) => ReactNode;
  packs: (next: () => void, label: string) => ReactNode;
}) {
  const t = useTranslations('installation');
  const router = useRouter();
  const installation = useInstallation().data;
  const finish = useFinishFirstRun();
  const [step, setStep] = useState<Step>('secondStep');
  const [organisation, setOrganisation] = useState('');
  const [error, setError] = useState<ActionError | null>(null);

  async function finishWith(name: string) {
    setError(null);
    const result = await finish.mutateAsync({ organisation: name });
    if (result.ok) router.replace('/');
    else setError(result.error);
  }

  if (!installation) return <Skeleton className="h-96 w-full" />;
  if (!installation.ok) return <ErrorAlert error={installation.error} />;
  if (!installation.data.firstRun || !isAdmin) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{t(installation.data.firstRun ? 'adminsOnlyTitle' : 'doneTitle')}</CardTitle>
          <CardDescription>{t(installation.data.firstRun ? 'adminsOnly' : 'done')}</CardDescription>
        </CardHeader>
        <CardContent>
          <Button asChild>
            <Link href="/">{t('toStudio')}</Link>
          </Button>
        </CardContent>
      </Card>
    );
  }

  const position = STEPS.indexOf(step);
  return (
    <div className="mx-auto grid w-full max-w-2xl gap-6">
      <div className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('title')}</h1>
        <p className="text-muted-foreground">{t('description')}</p>
      </div>
      <ol aria-label={t('stepsLabel')} className="grid gap-2 sm:grid-cols-3">
        {STEPS.map((name, index) => (
          <li
            key={name}
            aria-current={index === position ? 'step' : undefined}
            className={cn(
              'flex items-center gap-2 rounded-md border px-3 py-2 text-sm',
              index === position ? 'border-primary font-medium' : 'text-muted-foreground',
            )}
          >
            <span
              className={cn(
                'flex size-6 shrink-0 items-center justify-center rounded-full border text-xs',
                index < position && 'border-primary bg-primary text-primary-foreground',
              )}
            >
              {index < position ? <Check aria-hidden className="size-3.5" /> : index + 1}
            </span>
            {t(`${name}.short`)}
            {index < position ? <span className="sr-only">{t('stepDone')}</span> : null}
          </li>
        ))}
      </ol>
      <Card>
        <CardHeader>
          <CardTitle>{t(`${step}.title`)}</CardTitle>
          <CardDescription>{t(`${step}.description`)}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          {error ? <ErrorAlert error={error} /> : null}
          {step === 'secondStep' ? secondStep(() => setStep('organisation'), t('continue')) : null}
          {step === 'organisation' ? (
            <OrganisationStep
              initial={organisation}
              onDone={(name) => {
                setOrganisation(name);
                setStep('packs');
              }}
            />
          ) : null}
          {step === 'packs' && !finish.isPending ? packs(() => void finishWith(organisation), t('finish')) : null}
          {finish.isPending ? <p className="text-sm text-muted-foreground">{t('finishing')}</p> : null}
        </CardContent>
      </Card>
    </div>
  );
}

function OrganisationStep({ initial, onDone }: { initial: string; onDone: (name: string) => void }) {
  const t = useTranslations('installation');
  const form = useAppForm({
    defaultValues: { organisation: initial } satisfies OrganisationValues,
    validators: { onChange: organisationSchema, onSubmit: organisationSchema },
    onSubmit: ({ value }) => onDone(organisationSchema.parse(value).organisation),
  });
  return (
    <form.AppForm>
      <form.Form className="grid gap-4 sm:max-w-md">
        <form.AppField name="organisation">
          {(field) => (
            <field.TextField
              label={t('organisationName')}
              hint={t('organisationHint')}
              autoComplete="organization"
              required
              autoFocus
              messageValues={{ max: ORGANISATION_MAX }}
            />
          )}
        </form.AppField>
        <form.SubmitButton pendingLabel={t('continue')} className="justify-self-start">
          {t('continue')}
        </form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}
