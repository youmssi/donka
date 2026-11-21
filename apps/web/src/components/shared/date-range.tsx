'use client';

import { CalendarRange } from 'lucide-react';
import { useLocale } from 'next-intl';
import { useState } from 'react';
import type { DateRange } from 'react-day-picker';
import { enGB, fr } from 'react-day-picker/locale';

import { useDateFormat } from '@/components/shared/format';
import { Button } from '@/components/ui/button';
import { Calendar } from '@/components/ui/calendar';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';

/**
 * Filters by a range of days. Addresses hold days (`YYYY-MM-DD`) in the
 * viewer's time zone, the last one included; the API takes instants, the
 * end exclusive.
 */
const DAY = /^(\d{4})-(\d{2})-(\d{2})$/;

/** The day, when `value` is one. */
export function readDay(value: string | null): string | undefined {
  return value && DAY.test(value) ? value : undefined;
}

/** `YYYY-MM-DD` of a day in the viewer's time zone. */
export function dayOf(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** Midnight of a `YYYY-MM-DD` day in the viewer's time zone. */
export function dateOf(day: string): Date {
  const [year, month, date] = day.split('-').map(Number) as [number, number, number];
  return new Date(year, month - 1, date);
}

/** The instants a range of days covers: `from` inclusive, `until` exclusive. */
export function rangeQuery(from: string | undefined, to: string | undefined): { from?: string; until?: string } {
  const startOf = (day: string, days: number) => {
    const start = dateOf(day);
    start.setDate(start.getDate() + days);
    return start.toISOString();
  };
  return {
    from: from && DAY.test(from) ? startOf(from, 0) : undefined,
    until: to && DAY.test(to) ? startOf(to, 1) : undefined,
  };
}

/** True when the last day comes before the first: nothing could match. */
export function isEmptyRange(from: string | undefined, to: string | undefined): boolean {
  return Boolean(from && to && to < from);
}

/** Pick a range of days (Popover + Calendar); the last day is included. */
export function DateRangeFilter({
  from,
  to,
  label,
  anyLabel,
  onChange,
}: {
  from: string | undefined;
  to: string | undefined;
  /** What the button filters, for screen readers. */
  label: string;
  /** Shown when no day is picked. */
  anyLabel: string;
  onChange: (from: string | undefined, to: string | undefined) => void;
}) {
  const locale = useLocale();
  const format = useDateFormat();
  const [open, setOpen] = useState(false);
  const selected: DateRange | undefined = from ? { from: dateOf(from), to: to ? dateOf(to) : undefined } : undefined;
  const text = from
    ? to && to !== from
      ? `${format.date(dateOf(from))} – ${format.date(dateOf(to))}`
      : format.date(dateOf(from))
    : anyLabel;

  function select(range: DateRange | undefined) {
    onChange(range?.from ? dayOf(range.from) : undefined, range?.to ? dayOf(range.to) : undefined);
    if (range?.from && range.to) setOpen(false);
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" aria-label={label} className="w-56 justify-start font-normal">
          <CalendarRange className="opacity-60" aria-hidden />
          <span className="truncate">{text}</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align="start">
        <Calendar
          mode="range"
          selected={selected}
          onSelect={select}
          defaultMonth={selected?.from}
          numberOfMonths={2}
          disabled={{ after: new Date() }}
          locale={locale === 'fr' ? fr : enGB}
        />
      </PopoverContent>
    </Popover>
  );
}
