import { useTranslations } from 'next-intl';
import { Suspense, type ReactNode } from 'react';

import { LanguageSwitch } from '../language-switch';
import { ThemeToggle } from '../theme-toggle';
import { Brand } from './brand';

/** Header with brand, language, theme and an optional account slot; the page below. */
export function AppShell({ account, children }: { account?: ReactNode; children: ReactNode }) {
  const t = useTranslations('app');
  return (
    <div className="flex min-h-dvh flex-col">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:bg-background focus:px-3 focus:py-2"
      >
        {t('skipToContent')}
      </a>
      <header className="border-b">
        <div className="mx-auto flex h-14 w-full max-w-6xl items-center gap-2 px-4">
          <Brand />
          <div className="ml-auto flex items-center gap-1">
            {/* useSearchParams needs a boundary in a static export. */}
            <Suspense>
              <LanguageSwitch />
            </Suspense>
            <ThemeToggle />
            {account}
          </div>
        </div>
      </header>
      <main id="main" className="mx-auto w-full max-w-6xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  );
}
