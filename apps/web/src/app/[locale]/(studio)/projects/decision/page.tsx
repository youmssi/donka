import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';

import { DecisionEditorPage } from '@/modules/decision';

export async function generateMetadata({ params }: PageProps<'/[locale]/projects/decision'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'project' });
  return { title: t('decisions') };
}

export default async function Page({ params }: PageProps<'/[locale]/projects/decision'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return <DecisionEditorPage />;
}
