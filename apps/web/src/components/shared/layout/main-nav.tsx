'use client';

import { useTranslations } from 'next-intl';

import { cn } from '@/components/shared/utils';
import { Link, usePathname } from '@/i18n/navigation';

/** Sections of Studio; People only for administrators. */
export function MainNav({ isAdmin }: { isAdmin: boolean }) {
  const t = useTranslations('nav');
  const pathname = usePathname();
  const items = [
    { href: '/', label: t('projects'), current: pathname === '/' || pathname.startsWith('/projects') },
    ...(isAdmin ? [{ href: '/people', label: t('people'), current: pathname.startsWith('/people') }] : []),
  ];
  return (
    <nav aria-label={t('label')} className="ml-1 flex items-center gap-1 sm:ml-4">
      {items.map((item) => (
        <Link
          key={item.href}
          href={item.href}
          aria-current={item.current ? 'page' : undefined}
          className={cn(
            'rounded-md px-2 py-1.5 text-sm font-medium transition-colors hover:bg-accent sm:px-3',
            item.current ? 'text-foreground' : 'text-muted-foreground',
          )}
        >
          {item.label}
        </Link>
      ))}
    </nav>
  );
}
