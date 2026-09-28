import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { addMember, getProject, listMembers } from './project.service';
import { ProjectMembersPage } from './project-members';
import type { Project } from './schema';

vi.mock('./project.service', () => ({
  getProject: vi.fn(),
  listMembers: vi.fn(),
  addMember: vi.fn(),
  changeRole: vi.fn(),
  removeMember: vi.fn(),
}));
const getProjectMock = vi.mocked(getProject);
const listMembersMock = vi.mocked(listMembers);
const addMemberMock = vi.mocked(addMember);

vi.mock('@/modules/identity', () => ({
  useCurrentUser: () => ({ id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' }),
}));

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
  search.params = new URLSearchParams({ id: 'p-1' });
  listMembersMock.mockResolvedValue({ ok: true, data: members });
});

it('lets an owner add members and change roles', async () => {
  getProjectMock.mockResolvedValue({ ok: true, data: project('owner') });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText('grace@bank.example')).toBeInTheDocument();
  expect(screen.getByText('(you)')).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: 'Role of grace@bank.example' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Remove grace@bank.example' })).toBeInTheDocument();
  expect(screen.getByRole('heading', { name: 'Add a member' })).toBeInTheDocument();
});

it('shows a viewer the members without any control', async () => {
  getProjectMock.mockResolvedValue({ ok: true, data: project('viewer') });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText('grace@bank.example')).toBeInTheDocument();
  expect(screen.getByText('Only owners can change members.')).toBeInTheDocument();
  expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Add a member' })).not.toBeInTheDocument();
});

it('keeps an archived project read-only, even for its owner', async () => {
  getProjectMock.mockResolvedValue({ ok: true, data: project('owner', '2026-09-28T10:00:00Z') });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText(/This project is archived/)).toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Add a member' })).not.toBeInTheDocument();
});

it('explains that a person needs an account before being added', async () => {
  getProjectMock.mockResolvedValue({ ok: true, data: project('owner') });
  addMemberMock.mockResolvedValue({ ok: false, error: { code: 'NO_SUCH_USER' } });
  renderWithProviders(<ProjectMembersPage />);
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Email'), 'nobody@bank.example');
  await user.click(screen.getByRole('button', { name: 'Add' }));
  expect(await screen.findByText(/Ask an administrator to invite them first/)).toBeInTheDocument();
  expect(addMemberMock).toHaveBeenCalledWith('p-1', { email: 'nobody@bank.example', role: 'viewer' });
});

it('says the project is not available to a non-member', async () => {
  getProjectMock.mockResolvedValue({ ok: false, error: { code: 'PROJECT_NOT_FOUND', requestId: 'r-1' } });
  renderWithProviders(<ProjectMembersPage />);
  expect(await screen.findByText('Project not available')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Back to projects' })).toHaveAttribute('href', '/');
  expect(listMembersMock).not.toHaveBeenCalled();
});
