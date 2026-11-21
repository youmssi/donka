import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ApprovalsPage } from '@/modules/release';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/approvals'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('approvals') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/approvals'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ApprovalsPage />;
}
