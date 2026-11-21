import type { Preview } from '@storybook/nextjs-vite';
import { NextIntlClientProvider } from 'next-intl';
import { useTheme } from 'next-themes';
import { Suspense, useEffect, useRef } from 'react';

import { PageSkeleton } from '../src/components/shared/page-skeleton';
import { Providers } from '../src/components/shared/providers';
import { TourProvider } from '../src/components/shared/tour';
import { storyMessages } from '../src/stories/fixtures';
import '../src/app/globals.css';
import './fonts.css';

const NOW = new Date('2026-10-19T12:00:00Z');
const ignoreSeen = () => {};

function PreviewAppearance({ locale, theme }: { locale: string; theme: string }) {
  const { setTheme } = useTheme();
  const appliedTheme = useRef<string | null>(null);
  useEffect(() => {
    document.documentElement.lang = locale;
    document.body.classList.add('font-sans');
  }, [locale]);
  useEffect(() => {
    // Apply toolbar changes once, so the in-story theme control can still be used.
    if (appliedTheme.current === theme) return;
    appliedTheme.current = theme;
    setTheme(theme);
  }, [theme, setTheme]);
  return null;
}

const preview: Preview = {
  initialGlobals: { locale: 'en', theme: 'light' },
  globalTypes: {
    locale: {
      toolbar: {
        icon: 'globe',
        items: [
          { value: 'en', title: 'English' },
          { value: 'fr', title: 'Français' },
        ],
        dynamicTitle: true,
      },
    },
    theme: {
      toolbar: {
        icon: 'paintbrush',
        items: [
          { value: 'light', title: 'Light' },
          { value: 'dark', title: 'Dark' },
        ],
        dynamicTitle: true,
      },
    },
  },
  parameters: { layout: 'padded', nextjs: { appDirectory: true, navigation: { pathname: '/projects' } } },
  decorators: [
    (Story, context) => {
      const locale = context.globals.locale === 'fr' ? 'fr' : 'en';
      const theme = context.globals.theme === 'dark' ? 'dark' : 'light';
      return (
        <NextIntlClientProvider
          key={`${context.id}:${locale}`}
          locale={locale}
          messages={storyMessages(locale)}
          now={NOW}
          timeZone="Africa/Lagos"
        >
          <Providers>
            <TourProvider seen={undefined} onSeen={ignoreSeen}>
              <PreviewAppearance locale={locale} theme={theme} />
              <Suspense fallback={<PageSkeleton />}>
                <Story />
              </Suspense>
            </TourProvider>
          </Providers>
        </NextIntlClientProvider>
      );
    },
  ],
};

export default preview;
