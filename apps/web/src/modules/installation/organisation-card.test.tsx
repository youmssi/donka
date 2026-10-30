import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { getInstallation, renameOrganisation } from './installation.service';
import { OrganisationCard } from './organisation-card';

vi.mock('./installation.service', () => ({ getInstallation: vi.fn(), renameOrganisation: vi.fn() }));

beforeEach(() => vi.clearAllMocks());

it('renames the organisation once the installation is set up', async () => {
  vi.mocked(getInstallation).mockResolvedValue({
    ok: true,
    data: { firstRun: false, organisation: 'Microfinance SA' },
  });
  vi.mocked(renameOrganisation).mockResolvedValue({
    ok: true,
    data: { firstRun: false, organisation: 'Crédit Mutuel SA' },
  });
  const user = userEvent.setup();
  renderWithProviders(<OrganisationCard />);
  const name = await screen.findByLabelText(/^Organisation name/);
  expect(name).toHaveValue('Microfinance SA');
  await user.clear(name);
  await user.type(name, 'x'.repeat(101));
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(await screen.findByText('Use at most 100 characters.')).toBeInTheDocument();
  expect(renameOrganisation).not.toHaveBeenCalled();

  await user.clear(name);
  await user.type(name, 'Crédit Mutuel SA');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  await waitFor(() => expect(renameOrganisation).toHaveBeenCalledWith({ organisation: 'Crédit Mutuel SA' }));
});

it('leaves naming to the first-run assistant until it is done', async () => {
  vi.mocked(getInstallation).mockResolvedValue({ ok: true, data: { firstRun: true, organisation: null } });
  renderWithProviders(<OrganisationCard />);
  expect(await screen.findByText('The first-run assistant names the organisation.')).toBeInTheDocument();
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
});
