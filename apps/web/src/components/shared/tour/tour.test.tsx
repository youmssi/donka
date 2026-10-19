import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { NextIntlClientProvider } from 'next-intl';
import { useMemo } from 'react';

import messages from '@/messages/en.json';

import { TourProvider, useCurrentTour, usePageTour, type PageTour } from './tour';

function Page() {
  const tour = useMemo<PageTour>(
    () => ({
      id: 'editor',
      steps: [
        { element: '[data-tour="first"]', title: 'First thing', description: 'Here it is.' },
        { element: '[data-tour="missing"]', title: 'Not on this page', description: 'Skipped.' },
        { title: 'Last thing', description: 'A note.' },
      ],
    }),
    [],
  );
  usePageTour(tour);
  const current = useCurrentTour();
  return (
    <>
      <button data-tour="first">Save</button>
      <button onClick={current.start} disabled={!current.available}>
        Replay
      </button>
    </>
  );
}

function renderPage(seen: string[] | undefined, onSeen = vi.fn()) {
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <TourProvider seen={seen} onSeen={onSeen}>
        <Page />
      </TourProvider>
    </NextIntlClientProvider>,
  );
  return onSeen;
}

beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
afterEach(() => {
  document.querySelector('.driver-popover-close-btn')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  vi.useRealTimers();
});

it('starts once for someone who has not seen it, and skipping counts as seen', async () => {
  const onSeen = renderPage([]);
  await act(() => vi.advanceTimersByTimeAsync(700));
  expect(await screen.findByRole('dialog', { name: 'First thing' })).toBeInTheDocument();
  expect(screen.getByText('1 of 3')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Next' }));
  // The step whose element is not on the page is skipped.
  expect(await screen.findByRole('dialog', { name: 'Last thing' })).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Skip the tour' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(onSeen).toHaveBeenCalledWith('editor');
});

it('does not start for someone who has seen it, who can replay it', async () => {
  renderPage(['editor']);
  await act(() => vi.advanceTimersByTimeAsync(700));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Replay' }));
  expect(await screen.findByRole('dialog', { name: 'First thing' })).toBeInTheDocument();
});

it('waits while what was seen is unknown', async () => {
  renderPage(undefined);
  await act(() => vi.advanceTimersByTimeAsync(700));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
