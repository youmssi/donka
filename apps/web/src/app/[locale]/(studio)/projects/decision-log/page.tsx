import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { DecisionLogPage } from '@/modules/decision-log';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/decision-log'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('decision-log') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/decision-log'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <DecisionLogPage />;
}
