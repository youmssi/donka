import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { ImportPackDialog } from './import-pack-dialog';
import { importPack, inspectPack, listPacks } from './pack.service';
import type { Imported, Pack } from './schema';

vi.mock('./pack.service', () => ({ listPacks: vi.fn(), inspectPack: vi.fn(), importPack: vi.fn() }));
const listMock = vi.mocked(listPacks);
const importMock = vi.mocked(importPack);

const retail: Pack = {
  key: 'retail-credit',
  name: { en: 'Retail credit', fr: 'Crédit aux particuliers' },
  description: { en: 'Salary loans', fr: 'Prêts sur salaire' },
  version: '1.2',
  market: 'CM',
  currency: 'XAF',
  entry: 'scorecard',
  decisions: ['affordability', 'scorecard'],
  scenarios: 9,
};
const treasury: Pack = { ...retail, key: 'sme-treasury', name: { en: 'SME treasury', fr: 'Trésorerie PME' } };
const imported = (failed = 0): Imported => ({
  project: {
    id: 'p-9',
    key: 'salary-advance',
    name: 'Salary advance',
    description: '',
    createdAt: '2026-10-09T09:00:00Z',
    archivedAt: null,
    role: 'owner',
  },
  tests: { passed: 9 - failed, failed, errors: 0 },
});

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockResolvedValue({ ok: true, data: [retail, treasury] });
});

async function open() {
  const user = userEvent.setup();
  renderWithProviders(<ImportPackDialog />);
  await user.click(screen.getByRole('button', { name: 'From a pack' }));
  return { user, dialog: await screen.findByRole('dialog') };
}

it('imports a pack of the catalogue under a name and key of one’s choice', async () => {
  importMock.mockResolvedValue({ ok: true, data: imported() });
  const { user, dialog } = await open();
  expect(await within(dialog).findAllByText('Version 1.2')).toHaveLength(2);
  expect(within(dialog).getAllByText('9 scenarios')).toHaveLength(2);
  const create = within(dialog).getByRole('button', { name: 'Create the project' });
  expect(create).toBeDisabled();

  await user.click(within(dialog).getByRole('radio', { name: /Retail credit/ }));
  // The pack proposes its name and key.
  expect(within(dialog).getByLabelText(/^Name/)).toHaveValue('Retail credit');
  expect(within(dialog).getByLabelText(/^Key/)).toHaveValue('retail-credit');
  await user.clear(within(dialog).getByLabelText(/^Name/));
  await user.type(within(dialog).getByLabelText(/^Name/), 'Salary advance');
  expect(within(dialog).getByLabelText(/^Key/)).toHaveValue('salary-advance');
  await user.click(create);

  await waitFor(() =>
    expect(importMock).toHaveBeenCalledWith('retail-credit', { name: 'Salary advance', key: 'salary-advance' }),
  );
  expect(router.push).toHaveBeenCalledWith('/projects/decisions?p=salary-advance');
});

it('reads a pack file first, then imports that file', async () => {
  vi.mocked(inspectPack).mockResolvedValue({ ok: true, data: treasury });
  importMock.mockResolvedValue({ ok: true, data: imported() });
  const { user, dialog } = await open();
  await user.click(within(dialog).getByRole('tab', { name: 'Pack file' }));
  const file = new File(['zip'], 'treasury.donka-pack.zip', { type: 'application/zip' });
  await user.upload(within(dialog).getByLabelText('Pack file', { selector: 'input' }), file);
  expect(await within(dialog).findByText('SME treasury')).toBeInTheDocument();
  expect(within(dialog).getByLabelText(/^Key/)).toHaveValue('sme-treasury');
  await user.click(within(dialog).getByRole('button', { name: 'Create the project' }));
  await waitFor(() => expect(importMock).toHaveBeenCalledWith(file, { name: 'SME treasury', key: 'sme-treasury' }));
});

it('says why a pack file cannot be used', async () => {
  vi.mocked(inspectPack).mockResolvedValue({
    ok: false,
    error: { code: 'INVALID_PACK', details: { reason: 'pack.json is missing' } },
  });
  const { user, dialog } = await open();
  await user.click(within(dialog).getByRole('tab', { name: 'Pack file' }));
  await user.upload(within(dialog).getByLabelText('Pack file', { selector: 'input' }), new File(['x'], 'x.zip'));
  expect(await within(dialog).findByText('This pack cannot be used.')).toBeInTheDocument();
  expect(within(dialog).getByText('pack.json is missing')).toBeInTheDocument();
  expect(within(dialog).getByRole('button', { name: 'Create the project' })).toBeDisabled();
});

it('points to a pack file when the installation offers no packs', async () => {
  listMock.mockResolvedValue({ ok: true, data: [] });
  const { dialog } = await open();
  expect(await within(dialog).findByText('No packs in this installation')).toBeInTheDocument();
});

it('keeps the dialog open with the reason when the key is taken', async () => {
  importMock.mockResolvedValue({ ok: false, error: { code: 'PROJECT_KEY_TAKEN' } });
  const { user, dialog } = await open();
  await user.click(within(dialog).getByRole('radio', { name: /SME treasury/ }));
  await user.click(within(dialog).getByRole('button', { name: 'Create the project' }));
  expect(await within(dialog).findByText('Another project already uses this key.')).toBeInTheDocument();
  expect(router.push).not.toHaveBeenCalled();
});
