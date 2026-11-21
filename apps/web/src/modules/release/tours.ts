'use client';

import { useTranslations } from 'next-intl';
import { useMemo } from 'react';

import { usePageTour, type PageTour } from '@/components/shared/tour';

/** The guided tour of releases (DNK-41), started once per person when the list is `ready`. */
export function useReleasesTour(ready: boolean) {
  const t = useTranslations('tours.releases');
  const tour = useMemo<PageTour>(
    () => ({
      id: 'releases',
      steps: [
        { element: '[data-tour="releases-new"]', title: t('new.title'), description: t('new.body') },
        { element: '[data-tour="releases-list"]', title: t('list.title'), description: t('list.body'), side: 'top' },
        { title: t('deploy.title'), description: t('deploy.body') },
        { title: t('approval.title'), description: t('approval.body') },
      ],
    }),
    [t],
  );
  usePageTour(tour, ready);
}

/** The guided tour of environments (DNK-41), started once per person when the cards are `ready`. */
export function useEnvironmentsTour(ready: boolean) {
  const t = useTranslations('tours.environments');
  const tour = useMemo<PageTour>(
    () => ({
      id: 'environments',
      steps: [
        { element: '[data-tour="env-staging"]', title: t('staging.title'), description: t('staging.body') },
        { element: '[data-tour="env-deploy"]', title: t('deploy.title'), description: t('deploy.body') },
        { element: '[data-tour="env-tokens"]', title: t('tokens.title'), description: t('tokens.body'), side: 'top' },
        { element: '[data-tour="env-production"]', title: t('production.title'), description: t('production.body') },
      ],
    }),
    [t],
  );
  usePageTour(tour, ready);
}
