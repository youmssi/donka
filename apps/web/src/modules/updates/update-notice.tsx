'use client';

import { ArrowUpCircle } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { useCurrentUser } from '@/modules/identity';

import type { Updates } from './schema';
import { useUpdates } from './useUpdates';

/** The release an administrator chose to hide, until a later one is published. */
const DISMISSED = 'donka.updates.dismissed';

function dismissed(): string | null {
  try {
    return window.localStorage.getItem(DISMISSED);
  } catch {
    return null;
  }
}

function dismiss(version: string) {
  try {
    window.localStorage.setItem(DISMISSED, version);
  } catch {
    // Without storage the notice simply comes back on the next page.
  }
}

/** Tells administrators that a newer Donka release is published (DNK-64). */
export function UpdateNotice() {
  const user = useCurrentUser();
  const result = useUpdates(user.isAdmin).data;
  if (!user.isAdmin || !result?.ok) return null;
  return <Notice updates={result.data} />;
}

export function Notice({ updates }: { updates: Updates }) {
  const t = useTranslations('updates');
  const newer = updates.newer;
  const [hidden, setHidden] = useState(() => (newer ? dismissed() === newer.version : false));
  if (!newer || hidden) return null;
  return (
    <Alert className="mb-6">
      <ArrowUpCircle aria-hidden />
      <AlertTitle>{t('title', { version: newer.version })}</AlertTitle>
      <AlertDescription>
        <p>{t('description', { current: updates.current })}</p>
        <div className="mt-2 flex flex-wrap gap-2">
          <Button asChild size="sm" variant="outline">
            <a href={newer.url} target="_blank" rel="noopener noreferrer">
              {t('notes')}
            </a>
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              dismiss(newer.version);
              setHidden(true);
            }}
          >
            {t('dismiss')}
          </Button>
        </div>
      </AlertDescription>
    </Alert>
  );
}
