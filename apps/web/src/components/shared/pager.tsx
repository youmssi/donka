'use client';

import { useTranslations } from 'next-intl';

import { cn } from '@/components/shared/utils';
import { Button } from '@/components/ui/button';
import { Link } from '@/i18n/navigation';

/** Previous / next links for an offset-paginated list. */
export function Pager({
  hrefFor,
  offset,
  pageSize,
  total,
}: {
  hrefFor: (offset: number) => string;
  offset: number;
  pageSize: number;
  total: number;
}) {
  const common = useTranslations('common');
  const from = offset + 1;
  const to = Math.min(offset + pageSize, total);
  const label = common('pageOf', { from, to, total });
  const atStart = offset === 0;
  const atEnd = to >= total;
  return (
    <nav className="flex items-center justify-between gap-3 text-sm" aria-label={label}>
      <span className="text-muted-foreground">{label}</span>
      <span className="flex gap-2">
        <Button
          asChild
          variant="outline"
          size="sm"
          aria-disabled={atStart}
          className={cn(atStart && 'pointer-events-none opacity-50')}
        >
          <Link href={hrefFor(Math.max(0, offset - pageSize))}>{common('previous')}</Link>
        </Button>
        <Button
          asChild
          variant="outline"
          size="sm"
          aria-disabled={atEnd}
          className={cn(atEnd && 'pointer-events-none opacity-50')}
        >
          <Link href={hrefFor(offset + pageSize)}>{common('next')}</Link>
        </Button>
      </span>
    </nav>
  );
}
