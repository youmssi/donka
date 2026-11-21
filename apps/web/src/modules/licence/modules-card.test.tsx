import { screen, within } from '@testing-library/react';

import { renderWithProviders } from '@/test/render';

import { getLicence } from './licence.service';
import { ModulesCard } from './modules-card';
import type { Licence } from './schema';

vi.mock('./licence.service', () => ({ getLicence: vi.fn() }));

const licence: Licence = {
  status: 'active',
  customer: 'Bank of Example',
  validFrom: '2026-01-01',
  validUntil: '2027-06-30',
  daysLeft: 200,
  expiring: false,
  noticeDays: 30,
  modules: ['analysis'],
  turnedOff: ['connectors'],
  allowsChanges: true,
  allowsProduction: true,
  refusal: null,
  reason: null,
};

it('says which modules are on, turned off, or outside the licence', async () => {
  vi.mocked(getLicence).mockResolvedValue({ ok: true, data: licence });
  renderWithProviders(<ModulesCard />);
  const item = async (name: string) => (await screen.findByText(name)).closest('li') as HTMLElement;
  expect(within(await item('Explanations')).getByText('Not in the licence')).toBeInTheDocument();
  expect(within(await item('Connectors')).getByText('Turned off')).toBeInTheDocument();
  expect(within(await item('Analysis')).getByText('On')).toBeInTheDocument();
  expect(screen.getByText('donka-admin modules enable')).toBeInTheDocument();
});
