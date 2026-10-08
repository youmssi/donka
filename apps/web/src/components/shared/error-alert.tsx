'use client';

import { CircleAlert } from 'lucide-react';
import { useTranslations } from 'next-intl';
import type { ReactNode } from 'react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';

import type { ActionError, ErrorCode } from './api';

/** Failures on our side: the reference helps support find them in the logs. */
const REPORTABLE: ReadonlySet<ErrorCode> = new Set(['UNEXPECTED', 'DATABASE_UNAVAILABLE']);

/** The wait a RATE_LIMITED answer gives, in seconds (a minute if it gives none). */
function retryAfterSeconds(details: unknown): number {
  if (details && typeof details === 'object' && 'retryAfterSeconds' in details) {
    const seconds = details.retryAfterSeconds;
    if (typeof seconds === 'number' && seconds > 0) return seconds;
  }
  return 60;
}

/** A failed action, in the reader's language, with the reference to quote if they report it. */
export function ErrorAlert({ error, title, action }: { error: ActionError; title?: string; action?: ReactNode }) {
  const t = useTranslations('errors');
  const message =
    error.code === 'RATE_LIMITED' ? t(error.code, { seconds: retryAfterSeconds(error.details) }) : t(error.code);
  return (
    <Alert variant="destructive">
      <CircleAlert aria-hidden />
      {title ? <AlertTitle>{title}</AlertTitle> : null}
      <AlertDescription>
        <p>{message}</p>
        {error.requestId && REPORTABLE.has(error.code) ? (
          <p className="font-mono text-xs">{t('withRequestId', { requestId: error.requestId })}</p>
        ) : null}
        {action}
      </AlertDescription>
    </Alert>
  );
}
