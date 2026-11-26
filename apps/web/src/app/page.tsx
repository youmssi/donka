import { routing } from '@/i18n/routing';

import { LocaleRedirect } from './locale-redirect';

/** `/` has no language: send the visitor to their browser's language, English otherwise. */
export default function RootPage() {
  return (
    <html lang={routing.defaultLocale}>
      <body>
        <LocaleRedirect />
        <noscript>
          <a href={`/${routing.defaultLocale}/`}>Donka Studio</a>
        </noscript>
      </body>
    </html>
  );
}
