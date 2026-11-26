import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { Settings } from './settings';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/settings'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('settings') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/settings'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <Settings />;
}
