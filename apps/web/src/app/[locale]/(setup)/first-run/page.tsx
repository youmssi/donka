import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { Assistant } from './assistant';

export async function generateMetadata({ params }: PageProps<'/[locale]/first-run'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'installation' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]/first-run'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <Assistant />;
}
