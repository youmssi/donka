import { setRequestLocale } from 'next-intl/server';

import { AppShell } from '@/components/shared/layout/app-shell';
import { CenteredColumn } from '@/components/shared/layout/centered-column';

/** Pages reachable without a session: sign-in and the password pages. */
export default async function PublicLayout({ children, params }: LayoutProps<'/[locale]'>) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <AppShell>
      <CenteredColumn>{children}</CenteredColumn>
    </AppShell>
  );
}
