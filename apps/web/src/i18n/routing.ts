import { defineRouting } from 'next-intl/routing';

// Every page lives under its locale (/en/…, /fr/…): the static export has no
// server to negotiate the language.
export const routing = defineRouting({
  locales: ['en', 'fr'],
  defaultLocale: 'en',
  localePrefix: 'always',
});

export type Locale = (typeof routing.locales)[number];
