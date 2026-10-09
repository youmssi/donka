'use client';

import 'driver.js/dist/driver.css';

import { driver } from 'driver.js';
import { useTranslations } from 'next-intl';
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

/** One step of a guided tour: a highlighted element, or a centred note when `element` is unset. */
export interface TourStep {
  /** CSS selector, usually `[data-tour="…"]`. A step whose element is missing is skipped. */
  element?: string;
  title: string;
  description: string;
  side?: 'top' | 'right' | 'bottom' | 'left';
}

/** The tour of the page on screen. */
export interface PageTour {
  id: string;
  steps: TourStep[];
}

interface TourContextValue {
  /** Tours already seen; `null` while unknown, when no tour starts on its own. */
  seen: ReadonlySet<string> | null;
  onSeen: (id: string) => void;
  current: PageTour | null;
  setCurrent: (tour: PageTour | null) => void;
}

const TourContext = createContext<TourContextValue | null>(null);

/**
 * Holds the tours a person has seen (kept on the server by the caller) and the tour of the page
 * on screen, which the Help menu replays.
 */
export function TourProvider({
  seen,
  onSeen,
  children,
}: {
  seen: readonly string[] | undefined;
  onSeen: (id: string) => void;
  children: ReactNode;
}) {
  const [current, setCurrent] = useState<PageTour | null>(null);
  const value = useMemo(
    () => ({ seen: seen ? new Set(seen) : null, onSeen, current, setCurrent }),
    [seen, onSeen, current],
  );
  return <TourContext.Provider value={value}>{children}</TourContext.Provider>;
}

function useTourContext(): TourContextValue {
  const context = useContext(TourContext);
  if (!context) throw new Error('A tour needs a TourProvider above it.');
  return context;
}

/** Starts a tour; finishing, skipping or closing it counts as seen. */
function useRunTour() {
  const t = useTranslations('tour');
  const { onSeen } = useTourContext();
  return useCallback(
    (tour: PageTour) => {
      const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
      const run = driver({
        steps: tour.steps.map((step) => ({
          element: step.element,
          popover: { title: step.title, description: step.description, side: step.side, align: 'start' },
        })),
        animate: !reduceMotion,
        smoothScroll: !reduceMotion,
        skipMissingElement: true,
        showProgress: true,
        popoverClass: 'donka-tour',
        progressText: t('progress', { current: '{{current}}', total: '{{total}}' }),
        nextBtnText: t('next'),
        prevBtnText: t('previous'),
        doneBtnText: t('done'),
        onPopoverRender: (popover) => {
          popover.closeButton.setAttribute('aria-label', t('skip'));
          popover.closeButton.setAttribute('title', t('skip'));
          // Last in reading order (it stays in its corner), so focus lands on Next, not on Skip.
          popover.wrapper.appendChild(popover.closeButton);
        },
        // Every way out (Done, the close button, Esc, a click outside) counts as seen, even mid-animation.
        onDestroyStarted: (_element, _step, { driver: running }) => {
          onSeen(tour.id);
          running.destroy();
        },
      });
      run.drive();
      return run;
    },
    [t, onSeen],
  );
}

/**
 * Makes `tour` the page's tour, replayable from the Help menu, and starts it once, the first
 * time this person opens the page (on any device), when `ready`.
 */
export function usePageTour(tour: PageTour, ready = true) {
  const { seen, setCurrent } = useTourContext();
  const runTour = useRunTour();
  const started = useRef(false);

  useEffect(() => {
    setCurrent(tour);
    return () => setCurrent(null);
  }, [tour, setCurrent]);

  useEffect(() => {
    if (!ready || !seen || seen.has(tour.id) || started.current) return;
    // Let the page settle (fonts, lazy panels) so the first highlight lands in place.
    const timer = window.setTimeout(() => {
      started.current = true;
      runTour(tour);
    }, 600);
    return () => window.clearTimeout(timer);
  }, [ready, seen, tour, runTour]);
}

/** The page's tour, if it has one, and a way to replay it (the Help menu). */
export function useCurrentTour(): { available: boolean; start: () => void } {
  const { current } = useTourContext();
  const runTour = useRunTour();
  return { available: current !== null, start: () => (current ? void runTour(current) : undefined) };
}
