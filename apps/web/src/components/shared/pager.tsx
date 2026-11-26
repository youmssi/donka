'use client';

import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { cn } from '@/components/shared/utils';
import { Pagination, PaginationContent, PaginationItem, PaginationLink } from '@/components/ui/pagination';
import { Link } from '@/i18n/navigation';

/** Previous / next links for an offset-paginated list, with where the reader is. */
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
  const disabled = 'pointer-events-none opacity-50';
  return (
    <Pagination aria-label={label} className="justify-between gap-3 text-sm">
      <span className="self-center text-muted-foreground">{label}</span>
      <PaginationContent>
        <PaginationItem>
          <PaginationLink
            asChild
            size="default"
            aria-disabled={atStart}
            tabIndex={atStart ? -1 : undefined}
            className={cn('gap-1 px-2.5', atStart && disabled)}
          >
            <Link href={hrefFor(Math.max(0, offset - pageSize))}>
              <ChevronLeft aria-hidden />
              <span>{common('previous')}</span>
            </Link>
          </PaginationLink>
        </PaginationItem>
        <PaginationItem>
          <PaginationLink
            asChild
            size="default"
            aria-disabled={atEnd}
            tabIndex={atEnd ? -1 : undefined}
            className={cn('gap-1 px-2.5', atEnd && disabled)}
          >
            <Link href={hrefFor(offset + pageSize)}>
              <span>{common('next')}</span>
              <ChevronRight aria-hidden />
            </Link>
          </PaginationLink>
        </PaginationItem>
      </PaginationContent>
    </Pagination>
  );
}
