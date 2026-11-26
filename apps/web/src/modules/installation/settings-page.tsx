'use client';

import { useTranslations } from 'next-intl';
import type { ReactNode } from 'react';

import { PageHeader } from '@/components/shared/layout/page-header';

import { OrganisationCard } from './organisation-card';

/** The installation's settings (administrators); `more` holds other modules' cards. */
export function SettingsPage({ more }: { more?: ReactNode }) {
  const t = useTranslations('installation');
  return (
    <div className="grid max-w-2xl gap-4">
      <PageHeader title={t('settingsTitle')} description={t('settingsDescription')} />
      <OrganisationCard />
      {more}
    </div>
  );
}
