import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { importPack, listPacks } from './pack.service';
import { PackPicker } from './pack-picker';
import type { Pack } from './schema';

vi.mock('./pack.service', () => ({ listPacks: vi.fn(), importPack: vi.fn() }));
vi.mock('@/modules/project', async (original) => ({
  ...(await original<typeof import('@/modules/project')>()),
  useProjectList: () => ({
    data: { ok: true, data: { items: [{ key: 'sme-treasury' }], total: 1 } },
  }),
}));
const importMock = vi.mocked(importPack);

const retail: Pack = {
  key: 'retail-credit',
  name: { en: 'Retail credit', fr: 'Crédit aux particuliers' },
  description: { en: 'Salary loans', fr: 'Prêts sur salaire' },
  version: '1.2',
  market: 'CM',
  currency: 'XAF',
  entry: 'scorecard',
  decisions: ['scorecard'],
  scenarios: 9,
};
const treasury: Pack = { ...retail, key: 'sme-treasury', name: { en: 'SME treasury', fr: 'Trésorerie PME' } };
const microloans: Pack = { ...retail, key: 'microloans', name: { en: 'Microloans', fr: 'Microcrédit' } };

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listPacks).mockResolvedValue({ ok: true, data: [retail, treasury, microloans] });
});

it('offers every pack, none ticked, and imports the chosen ones under their own key', async () => {
  importMock.mockResolvedValueOnce({ ok: true, data: {} as never });
  // Imported by an earlier attempt: nothing to do.
  importMock.mockResolvedValueOnce({ ok: false, error: { code: 'PROJECT_KEY_TAKEN' } });
  const onDone = vi.fn();
  const user = userEvent.setup();
  renderWithProviders(<PackPicker onDone={onDone} doneLabel="Finish" />);

  const retailBox = await screen.findByRole('checkbox', { name: /Retail credit/ });
  expect(retailBox).not.toBeChecked();
  // A pack already in Studio is shown so, and cannot be chosen again.
  const treasuryBox = screen.getByRole('checkbox', { name: /SME treasury/ });
  expect(treasuryBox).toBeChecked();
  expect(treasuryBox).toBeDisabled();
  expect(screen.getByText('Already in Studio')).toBeInTheDocument();

  await user.click(retailBox);
  await user.click(screen.getByRole('checkbox', { name: /Microloans/ }));
  await user.click(screen.getByRole('button', { name: 'Finish' }));
  await waitFor(() => expect(onDone).toHaveBeenCalled());
  expect(importMock).toHaveBeenCalledWith('retail-credit', { key: 'retail-credit', name: 'Retail credit' });
  expect(importMock).toHaveBeenCalledWith('microloans', { key: 'microloans', name: 'Microloans' });
  expect(importMock).toHaveBeenCalledTimes(2);
});

it('goes on without packs, and stops on a failed import', async () => {
  const onDone = vi.fn();
  const user = userEvent.setup();
  renderWithProviders(<PackPicker onDone={onDone} doneLabel="Finish" />);
  await user.click(await screen.findByRole('checkbox', { name: /Retail credit/ }));
  importMock.mockResolvedValue({
    ok: false,
    error: { code: 'LICENCE_INACTIVE' },
  });
  await user.click(screen.getByRole('button', { name: 'Finish' }));
  expect(await screen.findByRole('alert')).toBeInTheDocument();
  expect(onDone).not.toHaveBeenCalled();

  await user.click(screen.getByRole('checkbox', { name: /Retail credit/ }));
  await user.click(screen.getByRole('button', { name: 'Finish' }));
  await waitFor(() => expect(onDone).toHaveBeenCalled());
});
