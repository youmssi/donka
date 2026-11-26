import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useMemo } from 'react';

import { usePageTour, type PageTour } from '@/components/shared/tour';
import { renderWithProviders } from '@/test/render';

import { listToursSeen, markTourSeen } from './onboarding.service';
import { TourState } from './tour-state';

vi.mock('./onboarding.service', () => ({ listToursSeen: vi.fn(), markTourSeen: vi.fn() }));

function Page({ id }: { id: string }) {
  const tour = useMemo<PageTour>(() => ({ id, steps: [{ title: `Tour of ${id}`, description: '…' }] }), [id]);
  usePageTour(tour);
  return null;
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.mocked(listToursSeen).mockResolvedValue({ ok: true, data: ['releases'] });
  vi.mocked(markTourSeen).mockResolvedValue({ ok: true, data: null });
});
afterEach(() => vi.useRealTimers());

it('starts a tour the person has not seen and tells the server once it is closed', async () => {
  renderWithProviders(
    <TourState>
      <Page id="editor" />
    </TourState>,
  );
  await act(() => vi.advanceTimersByTimeAsync(700));
  expect(await screen.findByRole('dialog', { name: 'Tour of editor' })).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Skip the tour' }));
  expect(markTourSeen).toHaveBeenCalledWith('editor');
});

it('does not start a tour the server says was seen, on another device', async () => {
  renderWithProviders(
    <TourState>
      <Page id="releases" />
    </TourState>,
  );
  await act(() => vi.advanceTimersByTimeAsync(700));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(markTourSeen).not.toHaveBeenCalled();
});
