import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { StabilityPage } from '@/modules/stability';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/stability'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('stability') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/stability'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <StabilityPage />;
}
