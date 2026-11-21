import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { PeopleRequestsPage } from '@/modules/decision-log';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/people-requests'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('people-requests') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/people-requests'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <PeopleRequestsPage />;
}
