import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { toast } from 'sonner';

import { invite } from '@/modules/people/people.service';
import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { addMember, changeRole, getProject, getProjectByKey, listMembers } from './project.service';
import { ProjectMembersPage } from './project-members';
import type { Project } from './schema';

vi.mock('./project.service', () => ({
  getProject: vi.fn(),
  getProjectByKey: vi.fn(),
  listMembers: vi.fn(),
  addMember: vi.fn(),
  changeRole: vi.fn(),
  removeMember: vi.fn(),
}));
const getProjectMock = vi.mocked(getProject);
const byKeyMock = vi.mocked(getProjectByKey);
const listMembersMock = vi.mocked(listMembers);
const addMemberMock = vi.mocked(addMember);
const changeRoleMock = vi.mocked(changeRole);

vi.mock('@/modules/people/people.service', () => ({ invite: vi.fn(), listAccounts: vi.fn(), PAGE_SIZE: 50 }));
const inviteMock = vi.mocked(invite);

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

const me = { id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' as const };
vi.mock('@/modules/identity', () => ({ useCurrentUser: () => me }));

const project = (role: Project['role'], archivedAt: string | null = null): Project => ({
  id: 'p-1',
  key: 'retail',
  name: 'Retail scoring',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt,
  role,
});
const members = [
  { userId: 'u-1', email: 'ada@bank.example', role: 'owner' as const, addedAt: '2026-09-28T09:00:00Z' },
  { userId: 'u-2', email: 'grace@bank.example', role: 'viewer' as const, addedAt: '2026-09-28T09:00:00Z' },
];

beforeEach(() => {
  vi.clearAllMocks();
  me.isAdmin = true;
  search.params = new URLSearchParams({ p: 'retail' });
  listMembersMock.mockResolvedValue({ ok: true, data: members });
});

async function openAddDialog() {
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'Add a member' }));
  return { user, dialog: await screen.findByRole('dialog') };
}

it('shows an owner the members in a table, with actions per member', async () => {
  byKeyMock.mockResolvedValue({ ok: true, data: project('owner') });
  renderWithProviders(<ProjectMembersPage />);
  const table = await screen.findByRole('table', { name: 'Members' });
  expect(await within(table).findByText('grace@bank.example')).toBeInTheDocument();
  expect(within(table).getByText('(you)')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Actions for grace@bank.example' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Add a member' })).toBeInTheDocument();
  expect(byKeyMock).toHaveBeenCalledWith('retail');
});

it('changes a role from the member actions and confirms it', async () => {
  byKeyMock.mockResolvedValue({ ok: true, data: project('owner') });
  changeRoleMock.mockResolvedValue({
    ok: true,
    data: { userId: 'u-2', email: 'grace@bank.example', role: 'editor', addedAt: '2026-09-28T09:00:00Z' },
  });
  renderWithProviders(<ProjectMembersPage />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'Actions for grace@bank.example' }));
  await user.click(await screen.findByRole('menuitemradio', { name: 'Editor' }));
  expect(changeRoleMock).toHaveBeenCalledWith('p-1', 'u-2', 'editor');
  expect(toast.success).toHaveBeenCalledWith('grace@bank.example is now Editor.');
});

it('shows a viewer the members without any control', async () => {
  byKeyMock.mockResolvedValue({ ok: true, data: project('viewer') });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText('grace@bank.example')).toBeInTheDocument();
  expect(screen.getByText('Only owners can change members.')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Actions for/ })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Add a member' })).not.toBeInTheDocument();
});

it('keeps an archived project read-only, even for its owner', async () => {
  byKeyMock.mockResolvedValue({ ok: true, data: project('owner', '2026-09-28T10:00:00Z') });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText(/This project is archived/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Add a member' })).not.toBeInTheDocument();
});

it('tells an owner who is not an administrator to ask one for an invitation', async () => {
  me.isAdmin = false;
  byKeyMock.mockResolvedValue({ ok: true, data: project('owner') });
  addMemberMock.mockResolvedValue({ ok: false, error: { code: 'NO_SUCH_USER' } });
  renderWithProviders(<ProjectMembersPage />);
  const { user, dialog } = await openAddDialog();
  await user.type(within(dialog).getByLabelText('Email'), 'nobody@bank.example');
  await user.click(within(dialog).getByRole('button', { name: 'Add' }));
  expect(await within(dialog).findByText(/Ask an administrator to invite them first/)).toBeInTheDocument();
  expect(within(dialog).queryByRole('button', { name: /Invite nobody@bank.example/ })).not.toBeInTheDocument();
  expect(addMemberMock).toHaveBeenCalledWith('p-1', { email: 'nobody@bank.example', role: 'viewer' });
});

it('lets an administrator invite an unknown email and add it in one step', async () => {
  byKeyMock.mockResolvedValue({ ok: true, data: project('owner') });
  addMemberMock.mockResolvedValueOnce({ ok: false, error: { code: 'NO_SUCH_USER' } }).mockResolvedValueOnce({
    ok: true,
    data: { userId: 'u-3', email: 'alan@bank.example', role: 'viewer', addedAt: '2026-09-28T09:00:00Z' },
  });
  inviteMock.mockResolvedValue({
    ok: true,
    data: { id: 'u-3', email: 'alan@bank.example', isAdmin: false, locale: 'en' },
  });
  renderWithProviders(<ProjectMembersPage />);
  const { user, dialog } = await openAddDialog();
  await user.type(within(dialog).getByLabelText('Email'), 'alan@bank.example');
  await user.click(within(dialog).getByRole('button', { name: 'Add' }));
  expect(await within(dialog).findByText(/No Studio account uses alan@bank.example yet/)).toBeInTheDocument();
  await user.click(within(dialog).getByRole('button', { name: 'Invite alan@bank.example to Studio and add them' }));
  expect(inviteMock).toHaveBeenCalledWith({ email: 'alan@bank.example', locale: 'en', isAdmin: false });
  expect(addMemberMock).toHaveBeenLastCalledWith('p-1', { email: 'alan@bank.example', role: 'viewer' });
  expect(toast.success).toHaveBeenCalledWith(
    expect.stringMatching(/alan@bank.example was invited to Studio and added/),
  );
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});

it('says the project is not available to a non-member', async () => {
  byKeyMock.mockResolvedValue({ ok: false, error: { code: 'PROJECT_NOT_FOUND', requestId: 'r-1' } });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText('Project not available')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Back to projects' })).toHaveAttribute('href', '/');
  expect(listMembersMock).not.toHaveBeenCalled();
});

it('turns a link made with the project id into the short one', async () => {
  search.params = new URLSearchParams({ id: 'p-1' });
  getProjectMock.mockResolvedValue({ ok: true, data: project('owner') });
  renderWithProviders(<ProjectMembersPage />);
  await screen.findByRole('table', { name: 'Members' });
  expect(getProjectMock).toHaveBeenCalledWith('p-1');
  expect(router.replace).toHaveBeenCalledWith('/projects/members?p=retail');
});
