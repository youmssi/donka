import type { ReactNode } from 'react';

/** Narrow column for sign-in and password pages. */
export function CenteredColumn({ children }: { children: ReactNode }) {
  return <div className="mx-auto grid w-full max-w-sm gap-6 pt-4 sm:pt-12">{children}</div>;
}
