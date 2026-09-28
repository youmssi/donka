import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ProjectListPage } from '@/modules/project';

export async function generateMetadata({ params }: PageProps<'/[locale]'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'projects' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ProjectListPage />;
}
