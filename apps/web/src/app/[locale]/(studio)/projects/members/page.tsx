import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { ProjectMembersPage } from '@/modules/project';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/members'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('members') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/members'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <ProjectMembersPage />;
}
