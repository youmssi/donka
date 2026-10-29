'use client';

import { CalendarClock, FlaskConical, ShieldAlert } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { useDateFormat } from '@/components/shared/format';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { useCurrentUser } from '@/modules/identity';

import type { Licence } from './schema';
import { useLicence } from './useLicence';

/** A licence day (`YYYY-MM-DD`, UTC) at midnight where the viewer is, to format as a date. */
function licenceDay(day: string): Date {
  return new Date(`${day}T00:00:00`);
}

/**
 * What the licence means for this installation, above every page (DNK-77). Administrators are
 * told about evaluation and about a licence that ends soon; everyone is told when new versions
 * and releases wait for a valid licence.
 */
export function LicenceNotice() {
  const result = useLicence().data;
  const user = useCurrentUser();
  if (!result?.ok) return null;
  return <Notice licence={result.data} isAdmin={user.isAdmin} />;
}

export function Notice({ licence, isAdmin }: { licence: Licence; isAdmin: boolean }) {
  const t = useTranslations('licence');
  const format = useDateFormat();
  const day = (value: string | null | undefined) => (value ? format.date(licenceDay(value)) : '');
  const waiting = t('waiting');

  switch (licence.status) {
    case 'evaluation':
      if (!isAdmin) return null;
      return (
        <Alert className="mb-6">
          <FlaskConical aria-hidden />
          <AlertTitle>{t('evaluationTitle')}</AlertTitle>
          <AlertDescription>{t('evaluation')}</AlertDescription>
        </Alert>
      );
    case 'active':
      if (!isAdmin || !licence.expiring) return null;
      return (
        <Alert className="mb-6">
          <CalendarClock aria-hidden />
          <AlertTitle>{t('expiringTitle', { date: day(licence.validUntil) })}</AlertTitle>
          <AlertDescription>
            {licence.daysLeft ? t('expiring', { days: licence.daysLeft }) : t('lastDay')}
          </AlertDescription>
        </Alert>
      );
    case 'notYetValid':
      return (
        <Alert className="mb-6">
          <CalendarClock aria-hidden />
          <AlertTitle>{t('notYetValidTitle', { date: day(licence.validFrom) })}</AlertTitle>
          <AlertDescription>{waiting}</AlertDescription>
        </Alert>
      );
    case 'expired':
      return (
        <Alert variant="destructive" className="mb-6">
          <ShieldAlert aria-hidden />
          <AlertTitle>{t('expiredTitle', { date: day(licence.validUntil) })}</AlertTitle>
          <AlertDescription>
            <p>{waiting}</p>
            {isAdmin ? <p>{t('renew')}</p> : null}
          </AlertDescription>
        </Alert>
      );
    case 'refused':
      return (
        <Alert variant="destructive" className="mb-6">
          <ShieldAlert aria-hidden />
          <AlertTitle>{t('refusedTitle')}</AlertTitle>
          <AlertDescription>
            <p>{waiting}</p>
            {isAdmin && licence.refusal ? <p>{t(`refusal.${licence.refusal}`)}</p> : null}
          </AlertDescription>
        </Alert>
      );
  }
}
