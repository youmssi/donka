import { renderHook } from '@testing-library/react';
import { NextIntlClientProvider } from 'next-intl';
import type { ReactNode } from 'react';

import { useDateFormat } from './format';

function formatIn(locale: 'en' | 'fr') {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <NextIntlClientProvider locale={locale} messages={{}}>
      {children}
    </NextIntlClientProvider>
  );
  return renderHook(() => useDateFormat(), { wrapper }).result.current;
}

const now = new Date(2026, 8, 29, 16, 0);

it('writes dates short, with the year only when it is not this one', () => {
  const en = formatIn('en');
  expect(en.date(new Date(2026, 10, 13, 14, 5), now)).toBe('13 Nov');
  expect(en.date(new Date(2025, 10, 13), now)).toBe('13 Nov 2025');
  expect(en.time(new Date(2026, 10, 13, 14, 5))).toBe('14:05');
  expect(en.dateTime(new Date(2026, 10, 13, 14, 5), now)).toBe('13 Nov, 14:05');
  expect(formatIn('fr').date(new Date(2025, 10, 13), now)).toBe('13 nov. 2025');
});

it('says how long ago within a day, then the date and time', () => {
  const en = formatIn('en');
  expect(en.ago(new Date(2026, 8, 29, 15, 58), now)).toBe('2 min ago');
  expect(en.ago(new Date(2026, 8, 29, 14, 0), now)).toBe('2 hr ago');
  expect(en.ago(new Date(2026, 8, 27, 9, 30), now)).toBe('27 Sept, 09:30');
  // French puts a no-break space between the number and its unit.
  expect(formatIn('fr').ago(new Date(2026, 8, 29, 14, 0), now)).toMatch(/^il y a 2\sh$/);
});
