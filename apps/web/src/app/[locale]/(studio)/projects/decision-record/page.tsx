import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { RecordPage } from '@/modules/decision-log';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/decision-record'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('decision-record') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/decision-record'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <RecordPage />;
}
