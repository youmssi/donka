import { cn } from '@/components/shared/utils';

/**
 * The Donka mark: a D holding an input that runs into a decision node (docs/brand). The tile takes
 * its size and background from `className`; the glyph is drawn in the text colour. Wrapped in a
 * span so containers that size their direct `svg` children (sidebar buttons) leave it alone.
 */
export function BrandMark({ className }: { className?: string }) {
  return (
    <span aria-hidden className={cn('inline-grid shrink-0 overflow-hidden rounded-[23%]', className)}>
      <svg
        viewBox="0 0 64 64"
        className="size-full"
        fill="none"
        stroke="currentColor"
        strokeWidth={5}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M20 15 V49 M20 15 H30 C42 15 48 23 48 32 C48 41 42 49 30 49 H20 M20 32 H31" />
        <circle cx="35" cy="32" r="4.5" fill="currentColor" stroke="none" />
      </svg>
    </span>
  );
}
