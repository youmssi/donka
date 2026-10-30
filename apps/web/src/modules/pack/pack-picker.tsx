'use client';

import { useLocale, useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Skeleton } from '@/components/ui/skeleton';
import { useProjectList } from '@/modules/project';

import { PackSummary } from './pack-summary';
import { localized } from './schema';
import { useImportPack, usePacks } from './usePacks';

/**
 * Packs to start with (DNK-74's first run): the catalogue, none ticked. Each chosen pack becomes
 * a project under the pack's own key and name, owned by the administrator; one already in Studio
 * is shown as such and skipped. `onDone` follows once the chosen packs are imported.
 */
export function PackPicker({ onDone, doneLabel }: { onDone: () => void; doneLabel: string }) {
  const t = useTranslations('packs');
  const locale = useLocale();
  const packs = usePacks(true).data;
  const projects = useProjectList(false, 0).data;
  const existing = new Set(projects?.ok ? projects.data.items.map((project) => project.key) : []);
  const importPack = useImportPack();
  const [chosen, setChosen] = useState<string[]>([]);
  const [importing, setImporting] = useState<string | null>(null);
  const [error, setError] = useState<ActionError | null>(null);

  async function importChosen() {
    setError(null);
    for (const key of chosen) {
      const pack = packs?.ok ? packs.data.find((item) => item.key === key) : undefined;
      if (!pack || existing.has(key)) continue;
      setImporting(localized(pack.name, locale));
      const result = await importPack.mutateAsync({
        pack: key,
        values: { key, name: localized(pack.name, locale) },
      });
      // Already imported, e.g. by the demo or an earlier attempt: nothing to do.
      if (!result.ok && result.error.code !== 'PROJECT_KEY_TAKEN') {
        setImporting(null);
        setError(result.error);
        return;
      }
    }
    setImporting(null);
    onDone();
  }

  if (!packs) return <Skeleton className="h-40 w-full" />;
  if (!packs.ok) return <ErrorAlert error={packs.error} />;

  return (
    <div className="grid gap-4">
      {error ? <ErrorAlert error={error} /> : null}
      {packs.data.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('pickNone')}</p>
      ) : (
        <ul aria-label={t('catalogue')} className="grid gap-2">
          {packs.data.map((pack) => {
            const id = `pack-${pack.key}`;
            const present = existing.has(pack.key);
            return (
              <li key={pack.key} className="flex items-start gap-3 rounded-md border p-3">
                <Checkbox
                  id={id}
                  className="mt-1"
                  disabled={present || importing !== null}
                  checked={present || chosen.includes(pack.key)}
                  onCheckedChange={(value) =>
                    setChosen((current) =>
                      value === true ? [...current, pack.key] : current.filter((key) => key !== pack.key),
                    )
                  }
                />
                <label htmlFor={id} className="grid flex-1 gap-1">
                  <PackSummary pack={pack} />
                  {present ? (
                    <Badge variant="secondary" className="justify-self-start">
                      {t('alreadyInStudio')}
                    </Badge>
                  ) : null}
                </label>
              </li>
            );
          })}
        </ul>
      )}
      <Button
        type="button"
        className="justify-self-start"
        disabled={importing !== null}
        onClick={() => void importChosen()}
      >
        {importing ? t('importingPack', { name: importing }) : doneLabel}
      </Button>
    </div>
  );
}
