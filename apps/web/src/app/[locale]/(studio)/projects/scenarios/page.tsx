import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ScenariosPage } from '@/modules/decision';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/scenarios'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('scenarios') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/scenarios'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ScenariosPage />;
}
