'use client';

import { FolderOpen } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { useCurrentUser } from '@/modules/identity';

export function HomePage() {
  const t = useTranslations('home');
  const user = useCurrentUser();
  return (
    <div className="grid gap-6">
      <div className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('welcome')}</h1>
        <p className="text-muted-foreground">{t('signedInAs', { email: user.email })}</p>
      </div>
      {/* Empty state until projects arrive (DNK-7). */}
      <section className="grid place-items-center gap-2 rounded-xl border border-dashed px-6 py-12 text-center">
        <FolderOpen className="size-8 text-muted-foreground" aria-hidden />
        <h2 className="font-medium">{t('emptyTitle')}</h2>
        <p className="max-w-md text-sm text-muted-foreground">{t('empty')}</p>
      </section>
    </div>
  );
}
