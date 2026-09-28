import { useTranslations } from 'next-intl';

import { Link } from '@/i18n/navigation';

export function Brand() {
  const t = useTranslations('app');
  return (
    <Link href="/" className="flex items-center gap-2 rounded-md font-semibold tracking-tight">
      <span aria-hidden className="grid size-7 place-items-center rounded-md bg-primary text-primary-foreground">
        D
      </span>
      <span>{t('name')}</span>
    </Link>
  );
}
