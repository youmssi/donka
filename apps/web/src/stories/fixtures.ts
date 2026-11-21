import { useLocale } from 'next-intl';

import en from '@/messages/en.json';
import fr from '@/messages/fr.json';

export function storyMessages(locale: unknown): typeof en {
  return locale === 'fr' ? fr : en;
}

export function useStoryMessages() {
  return storyMessages(useLocale());
}

export const VIEW_STATES = ['loading', 'empty', 'error', 'success'] as const;
export type ViewState = (typeof VIEW_STATES)[number];
