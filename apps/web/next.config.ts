import type { NextConfig } from 'next';
import { PHASE_DEVELOPMENT_SERVER } from 'next/constants';
import createNextIntlPlugin from 'next-intl/plugin';

const withNextIntl = createNextIntlPlugin('./src/i18n/request.ts');

// Where `pnpm dev` forwards API calls, so the browser still talks to one origin.
const devApiOrigin = process.env.DONKA_DEV_API_ORIGIN ?? 'http://localhost:8080';

export default function config(phase: string): NextConfig {
  // Production builds are a static export served by apps/app (ADR-002). Rewrites
  // only exist in `next dev`, where Next itself serves the pages.
  if (phase === PHASE_DEVELOPMENT_SERVER) {
    return withNextIntl({
      async rewrites() {
        return [{ source: '/api/:path*', destination: `${devApiOrigin}/api/:path*` }];
      },
    });
  }
  return withNextIntl({
    output: 'export',
    trailingSlash: true,
    images: { unoptimized: true },
  });
}
