import type { Metadata } from 'next';
import { getTranslations, setRequestLocale } from 'next-intl/server';
import { Suspense } from 'react';

import { PageSkeleton } from '@/components/shared/page-skeleton';
import { SignInForm } from '@/modules/identity';

export async function generateMetadata({ params }: PageProps<'/[locale]/sign-in'>): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'signIn' });
  return { title: t('title') };
}

export default async function Page({ params }: PageProps<'/[locale]/sign-in'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <Suspense fallback={<PageSkeleton />}>
      <SignInForm />
    </Suspense>
  );
}
