import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useMemo } from 'react';

import { usePageTour, type PageTour } from '@/components/shared/tour';
import { renderWithProviders } from '@/test/render';

import { HelpMenu } from './help-menu';

function PageWithTour() {
  const tour = useMemo<PageTour>(
    () => ({ id: 'releases', steps: [{ title: 'Releases never change', description: 'Each one is frozen.' }] }),
    [],
  );
  usePageTour(tour);
  return null;
}

afterEach(() => {
  document.querySelector('.driver-popover-close-btn')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
});

it('says when the page has no tour', async () => {
  renderWithProviders(<HelpMenu />);
  await userEvent.click(screen.getByRole('button', { name: 'Help' }));
  expect(await screen.findByRole('menuitem', { name: 'No tour on this page' })).toHaveAttribute(
    'aria-disabled',
    'true',
  );
  expect(screen.getByRole('menuitem', { name: 'Get started checklist' })).toHaveAttribute('href', '/');
});

it('replays the tour of the page on screen', async () => {
  renderWithProviders(
    <>
      <PageWithTour />
      <HelpMenu />
    </>,
  );
  await userEvent.click(screen.getByRole('button', { name: 'Help' }));
  await userEvent.click(await screen.findByRole('menuitem', { name: 'Show the tour of this page' }));
  expect(await screen.findByRole('dialog', { name: 'Releases never change' })).toBeInTheDocument();
});
