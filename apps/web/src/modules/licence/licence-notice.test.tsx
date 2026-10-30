import { screen } from '@testing-library/react';

import { renderWithProviders } from '@/test/render';

import { Notice } from './licence-notice';
import type { Licence } from './schema';

const active: Licence = {
  status: 'active',
  customer: 'Bank of Example',
  validFrom: '2026-01-01',
  validUntil: '2027-06-30',
  daysLeft: 200,
  expiring: false,
  noticeDays: 30,
  modules: ['explain', 'connectors'],
  turnedOff: [],
  allowsChanges: true,
  allowsProduction: true,
  refusal: null,
  reason: null,
};

it('says nothing while the licence is valid and far from its end', () => {
  const { container } = renderWithProviders(<Notice licence={active} isAdmin />);
  expect(container).toBeEmptyDOMElement();
});

it('tells administrators, and only them, when the licence ends soon', () => {
  const ending = { ...active, validUntil: '2026-11-15', daysLeft: 12, expiring: true };
  renderWithProviders(<Notice licence={ending} isAdmin />);
  expect(screen.getByText(/^The licence ends on 15 Nov/)).toBeInTheDocument();
  expect(screen.getByText(/12 days left\./)).toBeInTheDocument();

  const { container } = renderWithProviders(<Notice licence={ending} isAdmin={false} />);
  expect(container).toBeEmptyDOMElement();
});

it('says the last day is today', () => {
  renderWithProviders(<Notice licence={{ ...active, daysLeft: 0, expiring: true }} isAdmin />);
  expect(screen.getByText(/Today is its last day\./)).toBeInTheDocument();
});

it('tells administrators that evaluation stops at staging', () => {
  const evaluation: Licence = {
    ...active,
    status: 'evaluation',
    customer: null,
    validFrom: null,
    validUntil: null,
    daysLeft: null,
    modules: ['explain', 'connectors', 'analysis'],
    allowsProduction: false,
  };
  renderWithProviders(<Notice licence={evaluation} isAdmin />);
  expect(screen.getByText('Evaluation, without a licence')).toBeInTheDocument();
  expect(screen.getByText(/releases cannot reach production/)).toBeInTheDocument();
});

it('tells everyone that new versions wait once the licence has ended', () => {
  const expired = { ...active, status: 'expired' as const, daysLeft: null, allowsChanges: false };
  renderWithProviders(<Notice licence={expired} isAdmin={false} />);
  expect(screen.getByRole('alert')).toHaveTextContent(/The licence ended on 30 Jun/);
  expect(screen.getByText(/Reading and exporting still work/)).toBeInTheDocument();
  expect(screen.queryByText(/Ask Donka for a new licence/)).not.toBeInTheDocument();
});

it('shows administrators why a licence file was refused', () => {
  const refused: Licence = {
    ...active,
    status: 'refused',
    customer: null,
    validFrom: null,
    validUntil: null,
    daysLeft: null,
    modules: [],
    allowsChanges: false,
    refusal: 'tampered',
    reason: 'its signature does not match its content: the file was changed after signing',
  };
  renderWithProviders(<Notice licence={refused} isAdmin />);
  expect(screen.getByText('The licence file was refused')).toBeInTheDocument();
  expect(screen.getByText(/It was changed after Donka signed it/)).toBeInTheDocument();

  renderWithProviders(<Notice licence={refused} isAdmin={false} />);
  expect(screen.getAllByText('The licence file was refused')).toHaveLength(2);
  expect(screen.getAllByText(/It was changed after Donka signed it/)).toHaveLength(1);
});
