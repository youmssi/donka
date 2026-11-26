'use client';

import { Sparkles } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Link } from '@/i18n/navigation';

import { useInstallation } from './useInstallation';

/** Asks an administrator to finish the first-run assistant, until someone does. */
export function FirstRunNotice({ isAdmin }: { isAdmin: boolean }) {
  const t = useTranslations('installation');
  const result = useInstallation().data;
  if (!isAdmin || !result?.ok || !result.data.firstRun) return null;
  return (
    <Alert className="mb-4">
      <Sparkles aria-hidden />
      <AlertTitle>{t('noticeTitle')}</AlertTitle>
      <AlertDescription className="grid gap-2">
        <p>{t('noticeText')}</p>
        <Button asChild size="sm" className="justify-self-start">
          <Link href="/first-run">{t('noticeAction')}</Link>
        </Button>
      </AlertDescription>
    </Alert>
  );
}
