'use client';

import { useEffect } from 'react';

import { routing, type Locale } from '@/i18n/routing';

function preferredLocale(): Locale {
  for (const language of navigator.languages) {
    const base = language.slice(0, 2).toLowerCase();
    const match = routing.locales.find((locale) => locale === base);
    if (match) return match;
  }
  return routing.defaultLocale;
}

export function LocaleRedirect() {
  useEffect(() => {
    window.location.replace(`/${preferredLocale()}/`);
  }, []);
  return null;
}
