import { setRequestLocale } from 'next-intl/server';
import { Suspense } from 'react';

import { SignedInFrame } from '@/modules/identity';
import { StudioShell } from '@/modules/workspace';

/** Every page in this group needs a session. */
export default async function StudioLayout({ children, params }: LayoutProps<'/[locale]'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <Suspense>
      <SignedInFrame>
        <StudioShell>{children}</StudioShell>
      </SignedInFrame>
    </Suspense>
  );
}
