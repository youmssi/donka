import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { issueToken, listTokens, revokeToken } from './decision-log.service';
import { RuntimesPage } from './runtimes-page';
import type { LogToken } from './schema';

vi.mock('./decision-log.service', async (original) => ({
  ...(await original<typeof import('./decision-log.service')>()),
  listTokens: vi.fn(),
  issueToken: vi.fn(),
  revokeToken: vi.fn(),
}));
const listMock = vi.mocked(listTokens);

const ada = { id: 'u-1', email: 'ada@bank.example' };
const live: LogToken = {
  id: 't-1',
  environment: 'production',
  name: 'runtime-prod-1',
  hint: 'Xy9z',
  createdBy: ada,
  createdAt: '2026-10-01T09:00:00Z',
  revokedBy: null,
  revokedAt: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockResolvedValue({ ok: true, data: [live] });
});

it('lists the tokens by name and hint, never their value', async () => {
  renderWithProviders(<RuntimesPage />);
  expect(await screen.findByText('runtime-prod-1')).toBeInTheDocument();
  expect(screen.getByText('…Xy9z')).toBeInTheDocument();
  expect(screen.getByText('Production')).toBeInTheDocument();
});

it('issues a token for an environment and shows it once with the Runtime settings', async () => {
  vi.mocked(issueToken).mockResolvedValue({
    ok: true,
    data: { ...live, id: 't-2', name: 'runtime-prod-2', token: 'dnk_log_secretvalue' },
  });
  renderWithProviders(<RuntimesPage />);
  await screen.findByText('runtime-prod-1');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'New token' }));
  const dialog = await screen.findByRole('dialog');
  // Production unless changed: the environment is chosen up front.
  expect(within(dialog).getByRole('combobox', { name: 'Environment' })).toHaveTextContent('Production');
  await user.type(within(dialog).getByRole('textbox', { name: /Name/ }), 'runtime-prod-2');
  await user.click(within(dialog).getByRole('button', { name: 'New token' }));
  expect(issueToken).toHaveBeenCalledWith({ environment: 'production', name: 'runtime-prod-2' });
  expect(await within(dialog).findByDisplayValue('dnk_log_secretvalue')).toBeInTheDocument();
  expect(within(dialog).getByText(/DECISION_LOG__TOKEN=dnk_log_secretvalue/)).toBeInTheDocument();
  expect(within(dialog).getByText(/only sends Production decisions/)).toBeInTheDocument();
});

it('revokes a token after confirming', async () => {
  vi.mocked(revokeToken).mockResolvedValue({ ok: true, data: null });
  renderWithProviders(<RuntimesPage />);
  await screen.findByText('runtime-prod-1');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Revoke' }));
  await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Revoke' }));
  expect(revokeToken).toHaveBeenCalledWith('t-1');
});

it('tells someone who is not an administrator why they cannot see the tokens', async () => {
  listMock.mockResolvedValue({ ok: false, error: { code: 'FORBIDDEN' } });
  renderWithProviders(<RuntimesPage />);
  expect(await screen.findByText('Only administrators manage Runtime tokens.')).toBeInTheDocument();
});
