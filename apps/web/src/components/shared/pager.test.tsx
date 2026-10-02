import { screen } from '@testing-library/react';

import { renderWithProviders } from '@/test/render';

import { Pager } from './pager';

const hrefFor = (offset: number) => `/projects/audit?p=retail&offset=${offset}`;

it('links to the previous and next pages and says where the reader is', () => {
  renderWithProviders(<Pager hrefFor={hrefFor} offset={50} pageSize={50} total={180} />);
  expect(screen.getByRole('navigation', { name: '51–100 of 180' })).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Previous' })).toHaveAttribute('href', '/projects/audit?p=retail&offset=0');
  expect(screen.getByRole('link', { name: 'Next' })).toHaveAttribute('href', '/projects/audit?p=retail&offset=100');
});

it('takes the unavailable direction out of reach on the first and last pages', () => {
  renderWithProviders(<Pager hrefFor={hrefFor} offset={0} pageSize={50} total={60} />);
  const previous = screen.getByRole('link', { name: 'Previous' });
  expect(previous).toHaveAttribute('aria-disabled', 'true');
  expect(previous).toHaveAttribute('tabindex', '-1');
  expect(screen.getByRole('link', { name: 'Next' })).toHaveAttribute('aria-disabled', 'false');
});
