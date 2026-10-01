import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { EnvironmentsPage } from '@/modules/release';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/environments'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('environments') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/environments'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <EnvironmentsPage />;
}
