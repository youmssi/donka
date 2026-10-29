import { fireEvent, screen } from '@testing-library/react';

import { renderWithProviders } from '@/test/render';

import type { Updates } from './schema';
import { Notice } from './update-notice';

const release = {
  version: '1.6.0',
  url: 'https://github.com/youmssi/donka/releases/tag/v1.6.0',
  publishedAt: '2026-10-20T09:00:00Z',
};
const newer: Updates = { current: '1.5.0', checking: true, newer: release };

beforeEach(() => window.localStorage.clear());

it('names the newer release, the installed one and its notes', () => {
  renderWithProviders(<Notice updates={newer} />);
  expect(screen.getByText('Donka 1.6.0 is available')).toBeInTheDocument();
  expect(screen.getByText(/This installation runs 1\.5\.0\./)).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Release notes' })).toHaveAttribute(
    'href',
    'https://github.com/youmssi/donka/releases/tag/v1.6.0',
  );
});

it('says nothing when the installation is up to date or does not check', () => {
  const { container } = renderWithProviders(<Notice updates={{ ...newer, newer: null }} />);
  expect(container).toBeEmptyDOMElement();
  const offline = renderWithProviders(<Notice updates={{ ...newer, checking: false, newer: null }} />);
  expect(offline.container).toBeEmptyDOMElement();
});

it('stays hidden for the release it was hidden for, and comes back for a later one', () => {
  const first = renderWithProviders(<Notice updates={newer} />);
  fireEvent.click(screen.getByRole('button', { name: 'Hide until the next release' }));
  expect(first.container).toBeEmptyDOMElement();
  first.unmount();

  const again = renderWithProviders(<Notice updates={newer} />);
  expect(again.container).toBeEmptyDOMElement();
  again.unmount();

  const later = { ...newer, newer: { ...release, version: '1.7.0' } };
  renderWithProviders(<Notice updates={later} />);
  expect(screen.getByText('Donka 1.7.0 is available')).toBeInTheDocument();
});
