import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { RuntimesPage } from '@/modules/decision-log';

export async function generateMetadata({ params }: PageProps<'/[locale]/runtimes'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'runtimes' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]/runtimes'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <RuntimesPage />;
}
