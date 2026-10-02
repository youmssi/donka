import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ApprovalPage } from '@/modules/release';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/approval'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('approval') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/approval'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ApprovalPage />;
}
