import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { CopyProject } from './copy-project';
import { duplicateProject, exportProject } from './pack.service';

vi.mock('./pack.service', () => ({ duplicateProject: vi.fn(), exportProject: vi.fn() }));
const user = { id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' as const };
vi.mock('@/modules/identity', () => ({ useCurrentUser: () => user }));

const project = (role: 'owner' | 'editor') => ({
  id: 'p-1',
  key: 'retail-credit',
  name: 'Retail credit',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});
const releases = [{ id: 'r-2', version: '1.1.0' }];

beforeEach(() => {
  vi.clearAllMocks();
  user.isAdmin = true;
});

it('duplicates the project from a release', async () => {
  vi.mocked(duplicateProject).mockResolvedValue({
    ok: true,
    data: {
      project: { ...project('owner'), id: 'p-2', key: 'salary-advance' },
      tests: { passed: 3, failed: 0, errors: 0 },
    },
  });
  const events = userEvent.setup();
  renderWithProviders(<CopyProject project={project('owner')} releases={releases} />);
  await events.click(screen.getByRole('combobox', { name: 'Copy from' }));
  await events.click(await screen.findByRole('option', { name: 'Release 1.1.0' }));
  await events.click(screen.getByRole('button', { name: 'Duplicate' }));
  const dialog = await screen.findByRole('dialog');
  await events.type(within(dialog).getByLabelText(/^Name/), 'Salary advance');
  await events.click(within(dialog).getByRole('button', { name: 'Duplicate' }));
  await waitFor(() =>
    expect(duplicateProject).toHaveBeenCalledWith('p-1', { name: 'Salary advance', key: 'salary-advance' }, 'r-2'),
  );
  expect(router.push).toHaveBeenCalledWith('/projects/decisions?p=salary-advance');
});

it('exports the drafts as a pack file and shows a refusal in place', async () => {
  vi.mocked(exportProject).mockResolvedValue({
    ok: false,
    error: { code: 'INVALID_PACK', details: { reason: 'decision scorecard: invalid graph' } },
  });
  renderWithProviders(<CopyProject project={project('owner')} releases={releases} />);
  await userEvent.click(screen.getByRole('button', { name: 'Export pack file' }));
  expect(exportProject).toHaveBeenCalledWith('p-1', undefined);
  expect(await screen.findByText('decision scorecard: invalid graph')).toBeInTheDocument();
});

it('offers an editor who is not an administrator nothing', () => {
  user.isAdmin = false;
  renderWithProviders(<CopyProject project={project('editor')} releases={releases} />);
  expect(screen.queryByText('Copy or export')).not.toBeInTheDocument();
});

it('lets an owner who is not an administrator export, not duplicate', () => {
  user.isAdmin = false;
  renderWithProviders(<CopyProject project={project('owner')} releases={releases} />);
  expect(screen.getByRole('button', { name: 'Export pack file' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Duplicate' })).not.toBeInTheDocument();
});
