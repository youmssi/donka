'use client';

import { CircleCheck } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { Alert, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';

import { startEnrolment } from './identity.service';
import { EnrolmentSteps, RecoveryCodes } from './second-step';
import { useConfirmEnrolment, useRefreshSecondStep, useSecondStep } from './useSession';

/**
 * The signed-in person's second step, set up in place (DNK-74's first run): the QR code, a first
 * code, then the recovery codes. Already on, it says so. `onDone` follows either way.
 */
export function SecondStepSetup({ onDone, doneLabel }: { onDone: () => void; doneLabel: string }) {
  const t = useTranslations('secondStep');
  const status = useSecondStep().data;
  const confirm = useConfirmEnrolment();
  const refresh = useRefreshSecondStep();
  const [codes, setCodes] = useState<string[] | null>(null);

  if (codes) {
    return (
      <RecoveryCodes
        codes={codes}
        doneLabel={doneLabel}
        onDone={() => {
          refresh();
          onDone();
        }}
      />
    );
  }
  if (!status) return <Skeleton className="h-72 w-full" />;
  if (!status.ok) return <ErrorAlert error={status.error} />;
  if (status.data.enabled) {
    return (
      <div className="grid gap-4">
        <Alert>
          <CircleCheck aria-hidden className="text-success" />
          <AlertTitle>{t('onText')}</AlertTitle>
        </Alert>
        <Button type="button" className="justify-self-start" onClick={onDone}>
          {doneLabel}
        </Button>
      </div>
    );
  }
  return (
    <EnrolmentSteps start={startEnrolment} confirm={(values) => confirm.mutateAsync(values)} onConfirmed={setCodes} />
  );
}
