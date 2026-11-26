import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { DecisionsPage } from '@/modules/decision';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/decisions'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('decisions') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/decisions'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <DecisionsPage />;
}
