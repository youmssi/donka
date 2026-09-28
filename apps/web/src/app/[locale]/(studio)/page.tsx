import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { HomePage } from '@/modules/home';

export async function generateMetadata({ params }: PageProps<'/[locale]'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'home' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <HomePage />;
}
