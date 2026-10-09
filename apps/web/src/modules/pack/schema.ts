import type { ApiSchemas } from '@/components/shared/api';

export type Pack = ApiSchemas['PackResponse'];
export type Imported = ApiSchemas['ImportedResponse'];
export type LocalizedText = ApiSchemas['LocalizedText'];

/** Where a copy or an export is made from: each decision's draft, or the versions a release froze. */
export const DRAFTS = 'drafts';

/** A pack's text in the reader's language, English when it has none in French. */
export function localized(text: LocalizedText, locale: string): string {
  return locale === 'fr' && text.fr ? text.fr : text.en;
}

/** The release to copy from, or none for the drafts. */
export function releaseOf(source: string): string | undefined {
  return source === DRAFTS ? undefined : source;
}

/** The file name a download is saved under: the server's, from `Content-Disposition`. */
export function fileName(disposition: string | null, fallback: string): string {
  return disposition?.match(/filename="([^"]+)"/)?.[1] ?? fallback;
}
