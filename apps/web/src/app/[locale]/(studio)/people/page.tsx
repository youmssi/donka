import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { PeoplePage } from '@/modules/people';

export async function generateMetadata({ params }: PageProps<'/[locale]/people'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'people' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]/people'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <PeoplePage />;
}
