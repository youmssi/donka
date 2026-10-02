'use client';

import { useTranslations } from 'next-intl';
import { Fragment } from 'react';

import { Item, ItemActions, ItemContent, ItemGroup, ItemSeparator, ItemTitle } from '@/components/ui/item';
import { Link } from '@/i18n/navigation';
import { TestBadge } from '@/modules/decision';
import { decisionHref } from '@/modules/project';

import type { ReleasedDecision } from './schema';

/** The version of each decision a release freezes, with its tests; linked when `projectKey` is given. */
export function FrozenDecisions({ decisions, projectKey }: { decisions: ReleasedDecision[]; projectKey?: string }) {
  const t = useTranslations('releases');
  const title = t('frozen', { count: decisions.length });
  return (
    <section className="grid gap-2" aria-label={title}>
      <h3 className="text-sm font-medium">{title}</h3>
      <ItemGroup className="rounded-lg border">
        {decisions.map((decision, index) => (
          <Fragment key={decision.key}>
            {index > 0 ? <ItemSeparator /> : null}
            <Item size="sm" role="listitem">
              <ItemContent className="min-w-0">
                <ItemTitle className="w-full font-mono">
                  {projectKey ? (
                    <Link
                      href={decisionHref(projectKey, decision.key)}
                      className="min-w-0 truncate underline-offset-4 hover:underline"
                    >
                      {decision.key}
                    </Link>
                  ) : (
                    <span className="min-w-0 truncate">{decision.key}</span>
                  )}
                </ItemTitle>
              </ItemContent>
              <ItemActions>
                <TestBadge summary={decision.tests} />
                <span className="font-mono text-xs text-muted-foreground">v{decision.version}</span>
              </ItemActions>
            </Item>
          </Fragment>
        ))}
      </ItemGroup>
    </section>
  );
}
