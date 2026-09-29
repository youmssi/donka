import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { toast } from 'sonner';

import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { PeoplePage } from './people-page';
import { invite, listAccounts } from './people.service';

vi.mock('./people.service', () => ({ listAccounts: vi.fn(), invite: vi.fn(), PAGE_SIZE: 50 }));
const listMock = vi.mocked(listAccounts);
const inviteMock = vi.mocked(invite);

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

const accounts = [
  {
    id: 'u-1',
    email: 'ada@bank.example',
    isAdmin: true,
    locale: 'en' as const,
    active: true,
    createdAt: '2026-09-28T09:00:00Z',
  },
  {
    id: 'u-2',
    email: 'grace@bank.example',
    isAdmin: false,
    locale: 'fr' as const,
    active: false,
    createdAt: '2026-09-28T10:00:00Z',
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams();
});

it('lists accounts with their role and whether they are active', async () => {
  listMock.mockResolvedValue({ ok: true, data: { items: accounts, total: 2 } });
  renderWithProviders(<PeoplePage />);
  expect(await screen.findByText('grace@bank.example')).toBeInTheDocument();
  const [, ada, grace] = within(screen.getByRole('table', { name: 'People' })).getAllByRole('row');
  expect(ada).toHaveTextContent('Administrator');
  expect(ada).toHaveTextContent('Active');
  expect(grace).toHaveTextContent('Member');
  expect(grace).toHaveTextContent('Invited');
  // Only the pending invitation can be resent.
  expect(screen.getAllByRole('button', { name: /Resend the invitation/ })).toHaveLength(1);
});

it('resends a pending invitation with the same language and role', async () => {
  listMock.mockResolvedValue({ ok: true, data: { items: accounts, total: 2 } });
  inviteMock.mockResolvedValue({
    ok: true,
    data: { id: 'u-2', email: 'grace@bank.example', isAdmin: false, locale: 'fr' },
  });
  renderWithProviders(<PeoplePage />);
  await userEvent
    .setup()
    .click(await screen.findByRole('button', { name: 'Resend the invitation to grace@bank.example' }));
  expect(inviteMock).toHaveBeenCalledWith({ email: 'grace@bank.example', locale: 'fr', isAdmin: false });
  expect(toast.success).toHaveBeenCalledWith(
    expect.stringMatching(/A new invitation is on its way to grace@bank.example/),
  );
});

it('invites a person from the dialog', async () => {
  listMock.mockResolvedValue({ ok: true, data: { items: accounts, total: 2 } });
  inviteMock.mockResolvedValue({
    ok: true,
    data: { id: 'u-3', email: 'alan@bank.example', isAdmin: true, locale: 'en' },
  });
  renderWithProviders(<PeoplePage />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'Invite a person' }));
  await user.type(screen.getByLabelText('Email'), 'alan@bank.example');
  await user.click(screen.getByRole('checkbox', { name: 'Administrator' }));
  await user.click(screen.getByRole('button', { name: 'Send the invitation' }));
  expect(inviteMock).toHaveBeenCalledWith({ email: 'alan@bank.example', locale: 'en', isAdmin: true });
  expect(toast.success).toHaveBeenCalledWith('An invitation is on its way to alan@bank.example.');
});

it('says why a resend failed', async () => {
  listMock.mockResolvedValue({ ok: true, data: { items: accounts, total: 2 } });
  inviteMock.mockResolvedValue({ ok: false, error: { code: 'NETWORK' } });
  renderWithProviders(<PeoplePage />);
  await userEvent
    .setup()
    .click(await screen.findByRole('button', { name: 'Resend the invitation to grace@bank.example' }));
  expect(toast.error).toHaveBeenCalledWith('Studio could not be reached. Check your connection and try again.');
});

it('explains the page is for administrators', async () => {
  listMock.mockResolvedValue({ ok: false, error: { code: 'FORBIDDEN', requestId: 'r-1' } });
  renderWithProviders(<PeoplePage />);
  expect(await screen.findByText('Only administrators see this page')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Invite a person' })).not.toBeInTheDocument();
});
