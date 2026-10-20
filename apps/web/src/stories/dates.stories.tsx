import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { useLocale } from 'next-intl';
import { useState } from 'react';
import { enGB, fr } from 'react-day-picker/locale';

import { DateRangeFilter } from '@/components/shared/date-range';
import { When } from '@/components/shared/format';
import { Calendar } from '@/components/ui/calendar';

import { useStoryMessages } from './fixtures';

function DatesPreview({ kind }: { kind: 'calendar' | 'empty' | 'selected' | 'formats' }) {
  const m = useStoryMessages();
  const locale = useLocale();
  const [range, setRange] = useState<{ from?: string; to?: string }>(
    kind === 'selected' ? { from: '2026-10-01', to: '2026-10-19' } : {},
  );
  const [day, setDay] = useState<Date | undefined>(new Date(2026, 9, 19));
  if (kind === 'calendar')
    return (
      <Calendar
        mode="single"
        defaultMonth={new Date(2026, 9, 1)}
        selected={day}
        onSelect={setDay}
        locale={locale === 'fr' ? fr : enGB}
        className="rounded-lg border"
      />
    );
  if (kind === 'formats')
    return (
      <div className="grid gap-3">
        {(['date', 'time', 'dateTime', 'ago'] as const).map((as) => (
          <When key={as} value="2026-10-19T12:00:00Z" as={as} />
        ))}
      </div>
    );
  return (
    <DateRangeFilter
      from={range.from}
      to={range.to}
      label={m.decisionLog.dates}
      anyLabel={m.decisionLog.anyDate}
      onChange={(from, to) => setRange({ from, to })}
    />
  );
}

const meta = {
  title: 'Shared/Dates',
  component: DatesPreview,
  args: { kind: 'calendar' },
  render: (args) => <DatesPreview key={args.kind} {...args} />,
} satisfies Meta<typeof DatesPreview>;
export default meta;
type Story = StoryObj<typeof meta>;
export const CalendarView: Story = {};
export const EmptyRange: Story = { args: { kind: 'empty' } };
export const SelectedRange: Story = { args: { kind: 'selected' } };
export const Formats: Story = { args: { kind: 'formats' } };
