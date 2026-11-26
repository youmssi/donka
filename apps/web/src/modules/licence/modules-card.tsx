'use client';

import { useTranslations } from 'next-intl';

import { ErrorAlert } from '@/components/shared/error-alert';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';

import type { Licence, LicensedModule } from './schema';
import { useLicence } from './useLicence';

const MODULES: LicensedModule[] = ['explain', 'connectors', 'analysis'];

type State = 'on' | 'off' | 'unlicensed';

function stateOf(licence: Licence, module: LicensedModule): State {
  if (licence.modules.includes(module)) return 'on';
  return licence.turnedOff.includes(module) ? 'off' : 'unlicensed';
}

/**
 * The optional modules and whether they are on (DNK-74): within the licence, an operator turns
 * them on or off on the server; their data is kept either way.
 */
export function ModulesCard() {
  const t = useTranslations('licence.modules');
  const result = useLicence().data;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('title')}</CardTitle>
        <CardDescription>{t('description')}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {!result ? <Skeleton className="h-32 w-full" /> : null}
        {result && !result.ok ? <ErrorAlert error={result.error} /> : null}
        {result?.ok ? (
          <ul aria-label={t('title')} className="grid gap-3">
            {MODULES.map((module) => {
              const state = stateOf(result.data, module);
              return (
                <li key={module} className="flex items-start justify-between gap-3">
                  <span className="grid gap-0.5">
                    <span className="font-medium">{t(`${module}.name`)}</span>
                    <span className="text-sm text-muted-foreground">{t(`${module}.description`)}</span>
                  </span>
                  <Badge variant={state === 'on' ? 'secondary' : 'outline'} className="shrink-0">
                    {t(`state.${state}`)}
                  </Badge>
                </li>
              );
            })}
          </ul>
        ) : null}
        <p className="text-sm text-muted-foreground">
          {t.rich('howTo', { code: (chunks) => <code className="rounded bg-muted px-1 font-mono">{chunks}</code> })}
        </p>
      </CardContent>
    </Card>
  );
}
