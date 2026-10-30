'use client';

import { useTranslations } from 'next-intl';

import { PageHeader } from '@/components/shared/layout/page-header';

import { OrganisationCard } from './organisation-card';

/** The installation's settings (administrators). */
export function SettingsPage() {
  const t = useTranslations('installation');
  return (
    <div className="grid max-w-2xl gap-4">
      <PageHeader title={t('settingsTitle')} description={t('settingsDescription')} />
      <OrganisationCard />
    </div>
  );
}
