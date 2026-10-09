'use client';

import { useLocale, useTranslations } from 'next-intl';

import { Badge } from '@/components/ui/badge';

import { localized, type Pack } from './schema';

/** What a pack holds, in a line of badges: version, market, currency, decisions, scenarios. */
export function PackFacts({ pack }: { pack: Pack }) {
  const t = useTranslations('packs');
  return (
    <span className="flex flex-wrap gap-1.5">
      {pack.version ? <Badge variant="secondary">{t('version', { version: pack.version })}</Badge> : null}
      {pack.market ? <Badge variant="outline">{pack.market}</Badge> : null}
      {pack.currency ? <Badge variant="outline">{pack.currency}</Badge> : null}
      <Badge variant="outline">{t('decisionCount', { count: pack.decisions.length })}</Badge>
      <Badge variant="outline">{t('scenarioCount', { count: pack.scenarios })}</Badge>
    </span>
  );
}

/** A pack's name, description and facts. */
export function PackSummary({ pack }: { pack: Pack }) {
  const locale = useLocale();
  return (
    <span className="grid gap-1.5">
      <span className="font-medium">{localized(pack.name, locale)}</span>
      {localized(pack.description, locale) ? (
        <span className="text-sm text-muted-foreground">{localized(pack.description, locale)}</span>
      ) : null}
      <PackFacts pack={pack} />
    </span>
  );
}
