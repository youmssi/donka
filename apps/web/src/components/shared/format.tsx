'use client';

import { useLocale } from 'next-intl';
import { useMemo } from 'react';

import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

/**
 * English dates read day first ("13 Nov", "14:05"): Studio's users are in
 * Europe and Africa, where that is the norm.
 */
const DATE_LOCALES: Record<string, string> = { en: 'en-GB', fr: 'fr-FR' };

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The one way Studio writes dates and times, in the viewer's locale and time zone. */
export function useDateFormat() {
  const appLocale = useLocale();
  const locale = DATE_LOCALES[appLocale] ?? appLocale;
  return useMemo(() => {
    const dayMonth = new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' });
    const dayMonthYear = new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short', year: 'numeric' });
    const clock = new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' });
    const full = new Intl.DateTimeFormat(locale, {
      dateStyle: 'full',
      timeStyle: 'long',
    });
    const relative = new Intl.RelativeTimeFormat(locale, { numeric: 'auto', style: 'short' });

    /** "13 Nov" this year, "13 Nov 2025" otherwise. */
    const date = (value: Date, now = new Date()) =>
      (value.getFullYear() === now.getFullYear() ? dayMonth : dayMonthYear).format(value);
    /** "14:05". */
    const time = (value: Date) => clock.format(value);
    /** "13 Nov, 14:05". */
    const dateTime = (value: Date, now = new Date()) => `${date(value, now)}, ${time(value)}`;
    /** "now", "5 min ago", "2 hr ago" within a day, then as `dateTime`. */
    const ago = (value: Date, now = new Date()) => {
      const elapsed = now.getTime() - value.getTime();
      if (elapsed >= DAY || elapsed < -MINUTE) return dateTime(value, now);
      // A clock a few seconds ahead of the server's still reads "now".
      if (elapsed < MINUTE) return relative.format(0, 'second');
      if (elapsed < HOUR) return relative.format(-Math.round(elapsed / MINUTE), 'minute');
      return relative.format(-Math.floor(elapsed / HOUR), 'hour');
    };
    return { date, time, dateTime, ago, full: (value: Date) => full.format(value) };
  }, [locale]);
}

/** A date written short, with the full date, time and zone on hover. */
export function When({ value, as = 'date' }: { value: string; as?: 'date' | 'dateTime' | 'ago' }) {
  const format = useDateFormat();
  const at = new Date(value);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <time dateTime={value} className="whitespace-nowrap tabular-nums">
          {format[as](at)}
        </time>
      </TooltipTrigger>
      <TooltipContent>{format.full(at)}</TooltipContent>
    </Tooltip>
  );
}
