import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { SettingsPage } from '@/modules/installation';

export async function generateMetadata({ params }: PageProps<'/[locale]/settings'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'installation' });
  return { title: t('settingsTitle') };
}

export default async function Page({ params }: PageProps<'/[locale]/settings'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <SettingsPage />;
}
