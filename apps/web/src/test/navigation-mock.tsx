import type { ComponentProps } from 'react';
import { vi } from 'vitest';

/** Stand-ins for the locale-aware navigation and Next's search params. */
export const router = { replace: vi.fn(), push: vi.fn() };
export const search = { params: new URLSearchParams() };

vi.mock('@/i18n/navigation', () => ({
  Link: ({ href, ...props }: ComponentProps<'a'> & { href: string }) => <a href={href} {...props} />,
  useRouter: () => router,
  usePathname: () => '/',
}));

vi.mock('next/navigation', () => ({
  useSearchParams: () => search.params,
}));
