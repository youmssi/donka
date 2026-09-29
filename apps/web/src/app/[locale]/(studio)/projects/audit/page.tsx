import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ProjectAuditPage } from '@/modules/audit';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/audit'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'audit' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/audit'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ProjectAuditPage />;
}
