import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { CiTokens } from './ci-tokens';
import { issueCiToken, listCiTokens, revokeCiToken } from './release.service';
import type { CiToken } from './schema';

vi.mock('./release.service', async (original) => ({
  ...(await original<typeof import('./release.service')>()),
  listCiTokens: vi.fn(),
  issueCiToken: vi.fn(),
  revokeCiToken: vi.fn(),
}));
const listMock = vi.mocked(listCiTokens);

const project = (role: 'owner' | 'editor') => ({
  id: 'p-1',
  key: 'credit-pme',
  name: 'Crédit PME',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});
const ada = { id: 'u-1', email: 'ada@bank.example' };
const used: CiToken = {
  id: 't-1',
  name: 'loan-service deploy',
  hint: 'Q9xz',
  createdAt: '2026-10-01T09:00:00Z',
  createdBy: ada,
  revokedAt: null,
  revokedBy: null,
  lastUsedAt: '2026-10-02T09:00:00Z',
};

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockResolvedValue({ ok: true, data: [used, { ...used, id: 't-2', name: 'nightly', lastUsedAt: null }] });
});

it('lists the tokens by hint, with when a pipeline last used them', async () => {
  renderWithProviders(<CiTokens project={project('owner')} />);
  expect(await screen.findByText('loan-service deploy')).toBeInTheDocument();
  expect(screen.getAllByText('…Q9xz')).toHaveLength(2);
  expect(screen.getByText('never used')).toBeInTheDocument();
  expect(screen.getByText(/release:1.4.0, commit:RELEASE_ID/)).toBeInTheDocument();
});

it('issues a token and shows it once with the pipeline settings', async () => {
  vi.mocked(issueCiToken).mockResolvedValue({
    ok: true,
    data: { ...used, id: 't-3', name: 'release job', token: 'dnk_ci_secretvalue', lastUsedAt: null },
  });
  renderWithProviders(<CiTokens project={project('owner')} />);
  await screen.findByText('loan-service deploy');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'New CI token' }));
  const dialog = await screen.findByRole('dialog');
  await user.type(within(dialog).getByRole('textbox', { name: /Name/ }), 'release job');
  await user.click(within(dialog).getByRole('button', { name: 'New CI token' }));
  expect(issueCiToken).toHaveBeenCalledWith('p-1', 'release job');
  expect(await within(dialog).findByDisplayValue('dnk_ci_secretvalue')).toBeInTheDocument();
  expect(within(dialog).getByText(/DONKA_TOKEN=dnk_ci_secretvalue/)).toBeInTheDocument();
  expect(within(dialog).getByText(/DONKA_PROJECT=credit-pme/)).toBeInTheDocument();
});

it('revokes a token after confirming', async () => {
  vi.mocked(revokeCiToken).mockResolvedValue({ ok: true, data: null });
  renderWithProviders(<CiTokens project={project('owner')} />);
  await screen.findByText('loan-service deploy');
  const user = userEvent.setup();
  await user.click(screen.getAllByRole('button', { name: 'Revoke' })[0] as HTMLElement);
  await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Revoke' }));
  expect(revokeCiToken).toHaveBeenCalledWith('p-1', 't-1');
});

it('lets other members see the tokens but not manage them', async () => {
  renderWithProviders(<CiTokens project={project('editor')} />);
  await screen.findByText('loan-service deploy');
  expect(screen.queryByRole('button', { name: 'New CI token' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Revoke' })).not.toBeInTheDocument();
});
