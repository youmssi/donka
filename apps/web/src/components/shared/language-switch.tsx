'use client';

import { Languages } from 'lucide-react';
import { useSearchParams } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { usePathname, useRouter } from '@/i18n/navigation';
import { routing, type Locale } from '@/i18n/routing';

export const LOCALE_NAMES: Record<Locale, 'english' | 'french'> = { en: 'english', fr: 'french' };

/** The current language, and a way to change it that stays on the same page, query included. */
export function useLocaleSwitch() {
  const locale = useLocale();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const router = useRouter();
  const change = (next: string) => {
    const query = searchParams.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, { locale: next as Locale });
  };
  return { locale, change };
}

/** Switches language and stays on the same page, query string included. */
export function LanguageSwitch() {
  const t = useTranslations('common');
  const { locale, change } = useLocaleSwitch();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" aria-label={t('language')}>
          <Languages aria-hidden />
          <span className="uppercase">{locale}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuLabel>{t('language')}</DropdownMenuLabel>
        <DropdownMenuRadioGroup value={locale} onValueChange={change}>
          {routing.locales.map((code) => (
            <DropdownMenuRadioItem key={code} value={code} lang={code}>
              {t(LOCALE_NAMES[code])}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
