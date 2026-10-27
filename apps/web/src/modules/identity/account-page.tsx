'use client';

import { KeyRound, ShieldCheck, ShieldOff } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import { ErrorAlert } from '@/components/shared/error-alert';
import { PageHeader } from '@/components/shared/layout/page-header';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Skeleton } from '@/components/ui/skeleton';

import { startEnrolment } from './identity.service';
import type { SecondStepStatus } from './schema';
import { CodeForm, EnrolmentSteps, RecoveryCodes } from './second-step';
import { useCurrentUser } from './signed-in-frame';
import { useConfirmEnrolment, useDisableSecondStep, useRegenerateRecoveryCodes, useSecondStep } from './useSession';

/** The signed-in person's own settings: for now, two-step sign-in (DNK-59). */
export function AccountPage() {
  const t = useTranslations('account');
  const user = useCurrentUser();
  return (
    <div className="grid max-w-2xl gap-4">
      <PageHeader title={t('title')} description={user.email} />
      <SecondStepCard />
    </div>
  );
}

function SecondStepCard() {
  const t = useTranslations('secondStep');
  const common = useTranslations('common');
  const status = useSecondStep();
  const result = status.data;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {t('title')}
          {result?.ok ? (
            <Badge variant={result.data.enabled ? 'secondary' : 'outline'}>
              {result.data.enabled ? t('on') : t('off')}
            </Badge>
          ) : null}
        </CardTitle>
        <CardDescription>{t('description')}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3 text-sm">
        {!result ? <Skeleton className="h-10 w-full" /> : null}
        {result && !result.ok ? (
          <ErrorAlert
            error={result.error}
            action={
              <Button variant="outline" size="sm" className="mt-2" onClick={() => void status.refetch()}>
                {common('retry')}
              </Button>
            }
          />
        ) : null}
        {result?.ok ? <StatusText status={result.data} /> : null}
      </CardContent>
      {result?.ok ? (
        <CardFooter className="flex flex-wrap gap-2">
          {result.data.enabled ? (
            <>
              <RegenerateDialog />
              <DisableDialog required={result.data.required} />
            </>
          ) : (
            <EnableDialog />
          )}
        </CardFooter>
      ) : null}
    </Card>
  );
}

function StatusText({ status }: { status: SecondStepStatus }) {
  const t = useTranslations('secondStep');
  if (!status.enabled) {
    return <p className="text-muted-foreground">{status.required ? t('requiredOff') : t('offText')}</p>;
  }
  return (
    <div className="grid gap-1 text-muted-foreground">
      <p>{t('onText')}</p>
      <p className={status.recoveryCodesLeft <= 2 ? 'text-destructive' : undefined}>
        {status.recoveryCodesLeft === 0 ? t('noCodesLeft') : t('codesLeft', { count: status.recoveryCodesLeft })}
      </p>
      {status.required ? <p>{t('requiredOn')}</p> : null}
    </div>
  );
}

function EnableDialog() {
  const t = useTranslations('secondStep');
  const confirm = useConfirmEnrolment();
  const [open, setOpen] = useState(false);
  const [codes, setCodes] = useState<string[] | null>(null);

  function onOpenChange(next: boolean) {
    // Closing while the codes are shown would lose them: only the button closes then.
    if (!next && codes) return;
    setOpen(next);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button>
          <ShieldCheck aria-hidden />
          {t('enable')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{codes ? t('codesTitle') : t('enrolTitle')}</DialogTitle>
          <DialogDescription>{codes ? t('codesDescription') : t('enrolDescription')}</DialogDescription>
        </DialogHeader>
        {open && !codes ? (
          <EnrolmentSteps
            start={startEnrolment}
            confirm={(values) => confirm.mutateAsync(values)}
            onConfirmed={setCodes}
          />
        ) : null}
        {codes ? (
          <RecoveryCodes
            codes={codes}
            doneLabel={t('done')}
            onDone={() => {
              setCodes(null);
              setOpen(false);
              toast.success(t('enabled'));
            }}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

function RegenerateDialog() {
  const t = useTranslations('secondStep');
  const regenerate = useRegenerateRecoveryCodes();
  const [open, setOpen] = useState(false);
  const [codes, setCodes] = useState<string[] | null>(null);

  return (
    <Dialog open={open} onOpenChange={(next) => (!next && codes ? undefined : setOpen(next))}>
      <DialogTrigger asChild>
        <Button variant="outline">
          <KeyRound aria-hidden />
          {t('regenerate')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{codes ? t('codesTitle') : t('regenerateTitle')}</DialogTitle>
          <DialogDescription>{codes ? t('codesDescription') : t('regenerateDescription')}</DialogDescription>
        </DialogHeader>
        {codes ? (
          <RecoveryCodes
            codes={codes}
            doneLabel={t('done')}
            onDone={() => {
              setCodes(null);
              setOpen(false);
            }}
          />
        ) : (
          <CodeForm
            submitLabel={t('regenerateSubmit')}
            submit={async (values) => {
              const result = await regenerate.mutateAsync(values);
              if (!result.ok) return result.error;
              setCodes(result.data);
              return null;
            }}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function DisableDialog({ required }: { required: boolean }) {
  const t = useTranslations('secondStep');
  const disable = useDisableSecondStep();
  const [open, setOpen] = useState(false);

  if (required) {
    return (
      <Button variant="outline" disabled title={t('requiredOn')}>
        <ShieldOff aria-hidden />
        {t('disable')}
      </Button>
    );
  }
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline">
          <ShieldOff aria-hidden />
          {t('disable')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('disableTitle')}</DialogTitle>
          <DialogDescription>{t('disableDescription')}</DialogDescription>
        </DialogHeader>
        <CodeForm
          recovery
          submitLabel={t('disableSubmit')}
          submit={async (values) => {
            const result = await disable.mutateAsync(values);
            if (!result.ok) return result.error;
            setOpen(false);
            toast.success(t('disabled'));
            return null;
          }}
        />
      </DialogContent>
    </Dialog>
  );
}
