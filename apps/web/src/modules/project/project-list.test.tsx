import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { ProjectListPage } from './project-list';
import { listProjects } from './project.service';

vi.mock('./project.service', () => ({ listProjects: vi.fn(), PAGE_SIZE: 50 }));
const listMock = vi.mocked(listProjects);

const user = { id: 'u-1', email: 'ada@bank.example', isAdmin: false, locale: 'en' as const };
vi.mock('@/modules/identity', () => ({ useCurrentUser: () => user }));

beforeEach(() => {
  listMock.mockReset();
  search.params = new URLSearchParams();
  user.isAdmin = false;
});

it('tells a member without projects whom to ask', async () => {
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<ProjectListPage />);
  expect(await screen.findByText(/Ask a project owner to add you/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'New project' })).not.toBeInTheDocument();
});

it('invites an administrator to create the first project', async () => {
  user.isAdmin = true;
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<ProjectListPage />);
  expect(await screen.findByText(/Create the first project/)).toBeInTheDocument();
  expect(screen.getAllByRole('button', { name: 'New project' }).length).toBeGreaterThan(0);
});

it('lists projects with the reader role and links to them', async () => {
  listMock.mockResolvedValue({
    ok: true,
    data: {
      items: [
        { id: 'p-1', key: 'retail', name: 'Retail scoring', description: 'Loans', archivedAt: null, role: 'editor' },
      ],
      total: 1,
    },
  });
  renderWithProviders(<ProjectListPage />);
  const link = await screen.findByRole('link', { name: 'Retail scoring' });
  // Projects open by key: the link is short and readable.
  expect(link).toHaveAttribute('href', '/projects/decisions?p=retail');
  const row = link.closest('tr');
  expect(row).toHaveTextContent('retail');
  expect(row).toHaveTextContent('Editor');
  expect(screen.getByRole('table', { name: 'Projects' })).toBeInTheDocument();
  expect(listMock).toHaveBeenCalledWith(false, 0);
});

it('shows archived projects on the archived tab', async () => {
  search.params = new URLSearchParams({ view: 'archived' });
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<ProjectListPage />);
  expect(await screen.findByText('No archived projects')).toBeInTheDocument();
  expect(listMock).toHaveBeenCalledWith(true, 0);
  expect(screen.getByRole('tab', { name: 'Archived' })).toHaveAttribute('aria-selected', 'true');
});

it('switches between active and archived through the address', async () => {
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<ProjectListPage />);
  await screen.findByText('No projects yet');
  await userEvent.click(screen.getByRole('tab', { name: 'Archived' }));
  expect(router.replace).toHaveBeenLastCalledWith('/?view=archived');
});

it('offers a retry when the list cannot load', async () => {
  listMock.mockResolvedValue({ ok: false, error: { code: 'NETWORK' } });
  renderWithProviders(<ProjectListPage />);
  expect(await screen.findByText('We could not load your projects')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
});
