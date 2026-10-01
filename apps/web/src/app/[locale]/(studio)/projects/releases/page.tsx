import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ReleasesPage } from '@/modules/release';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/releases'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('releases') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/releases'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ReleasesPage />;
}
