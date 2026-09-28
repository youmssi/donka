import { useTranslations } from 'next-intl';

import { Skeleton } from '@/components/ui/skeleton';

/** Loading state of a page: the shape of what is coming, announced to screen readers. */
export function PageSkeleton() {
  const t = useTranslations('common');
  return (
    <div role="status" aria-live="polite" className="grid gap-4">
      <span className="sr-only">{t('loading')}</span>
      <Skeleton className="h-8 w-48" />
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-24 w-full" />
    </div>
  );
}
