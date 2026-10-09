'use client';

import { useTranslations } from 'next-intl';
import { useMemo } from 'react';

import { usePageTour, type PageTour } from '@/components/shared/tour';

/** The guided tour of the decision editor (DNK-41), started once per person. */
export function useEditorTour() {
  const t = useTranslations('tours.editor');
  const tour = useMemo<PageTour>(
    () => ({
      id: 'editor',
      steps: [
        { element: '[data-tour="editor-graph"]', title: t('graph.title'), description: t('graph.body'), side: 'top' },
        { element: '[data-tour="editor-status"]', title: t('status.title'), description: t('status.body') },
        { element: '[data-tour="editor-fields"]', title: t('fields.title'), description: t('fields.body') },
        { element: '[data-tour="editor-history"]', title: t('history.title'), description: t('history.body') },
        { element: '[data-tour="editor-save"]', title: t('save.title'), description: t('save.body') },
      ],
    }),
    [t],
  );
  usePageTour(tour);
}
