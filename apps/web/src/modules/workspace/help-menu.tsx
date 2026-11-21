'use client';

import { CircleHelp, ListChecks, Signpost } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { useCurrentTour } from '@/components/shared/tour';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Link } from '@/i18n/navigation';

/** Help: replay the tour of the page on screen, or go back to the Get started checklist. */
export function HelpMenu() {
  const t = useTranslations('help');
  const tour = useCurrentTour();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label={t('label')}>
          <CircleHelp aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-60">
        <DropdownMenuLabel>{t('label')}</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem disabled={!tour.available} onSelect={tour.start}>
          <Signpost aria-hidden />
          {tour.available ? t('tour') : t('noTour')}
        </DropdownMenuItem>
        <DropdownMenuItem asChild>
          <Link href="/">
            <ListChecks aria-hidden />
            {t('getStarted')}
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
