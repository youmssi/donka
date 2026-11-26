import { useTranslations } from 'next-intl';

import { BrandMark } from '@/components/shared/brand-mark';
import { Link } from '@/i18n/navigation';

export function Brand() {
  const t = useTranslations('app');
  return (
    <Link href="/" className="flex items-center gap-2 rounded-md font-semibold tracking-tight">
      <BrandMark className="size-7 bg-primary text-primary-foreground" />
      {/* The wordmark gives way on narrow screens so the navigation fits; the name stays readable. */}
      <span className="sr-only sm:not-sr-only">{t('name')}</span>
    </Link>
  );
}
