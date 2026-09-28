import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';
import { Suspense } from 'react';

import { PageSkeleton } from '@/components/shared/page-skeleton';
import { SetupPasswordForm } from '@/modules/identity';

export async function generateMetadata({ params }: PageProps<'/[locale]/setup-password'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'setupPassword' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]/setup-password'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <Suspense fallback={<PageSkeleton />}>
      <SetupPasswordForm />
    </Suspense>
  );
}
