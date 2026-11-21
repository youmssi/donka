import { setRequestLocale } from 'next-intl/server';
import { Suspense } from 'react';

import { AppShell } from '@/components/shared/layout/app-shell';
import { SignedInFrame } from '@/modules/identity';

/** Signed-in pages outside the workspace: the first-run assistant. */
export default async function SetupLayout({ children, params }: LayoutProps<'/[locale]'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <Suspense>
      <SignedInFrame>
        <AppShell>{children}</AppShell>
      </SignedInFrame>
    </Suspense>
  );
}
