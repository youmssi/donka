import Link from 'next/link';

import { routing } from '@/i18n/routing';

// Served by apps/app for any path that is not a page of the export. The language is
// unknown here, so the page speaks both.
export default function NotFound() {
  return (
    <html lang={routing.defaultLocale}>
      <body className="grid min-h-dvh place-items-center bg-background p-6 font-sans text-foreground">
        <main className="grid max-w-md gap-4 text-center">
          <h1 className="text-2xl font-semibold">
            Page not found · <span lang="fr">Page introuvable</span>
          </h1>
          <p className="text-muted-foreground">
            This page does not exist or has moved. ·{' '}
            <span lang="fr">Cette page n&apos;existe pas ou a été déplacée.</span>
          </p>
          <p className="flex justify-center gap-4">
            <Link className="text-primary underline" href="/en/">
              Go to Studio
            </Link>
            <Link className="text-primary underline" href="/fr/" lang="fr">
              Aller à Studio
            </Link>
          </p>
        </main>
      </body>
    </html>
  );
}
