import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { toast } from 'sonner';

import { getProjectByKey } from '@/modules/project/project.service';
import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { createRelease, listReleases, previewRelease } from './release.service';
import { ReleasesPage } from './releases-page';
import type { Release, ReleasePreview, ReleaseSummary } from './schema';

vi.mock('./release.service', () => ({
  RELEASES_PAGE_SIZE: 50,
  listReleases: vi.fn(),
  getRelease: vi.fn(),
  previewRelease: vi.fn(),
  createRelease: vi.fn(),
  deployRelease: vi.fn(),
}));
const listMock = vi.mocked(listReleases);
const previewMock = vi.mocked(previewRelease);
const createMock = vi.mocked(createRelease);
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock('@/modules/project/project.service', () => ({ getProject: vi.fn(), getProjectByKey: vi.fn() }));
const projectMock = vi.mocked(getProjectByKey);

const project = (role: 'owner' | 'editor' | 'viewer') => ({
  id: 'p-1',
  key: 'credit-pme',
  name: 'Crédit PME',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});
const ada = { id: 'u-1', email: 'ada@bank.example' };
const green = { passed: 2, failed: 0, errors: 0 };
const first: ReleaseSummary = {
  id: 'r-1',
  version: '1.0.0',
  notes: 'First scoring rules',
  createdAt: '2026-09-30T09:00:00Z',
  createdBy: ada,
  decisions: 1,
  tests: green,
  liveIn: ['staging'],
};
const preview = (overrides: Partial<ReleasePreview> = {}): ReleasePreview => ({
  latest: '1.0.0',
  next: { major: '2.0.0', minor: '1.1.0', patch: '1.0.1' },
  decisions: [{ decisionId: 'd-1', key: 'bureau/normalize', version: 3, tests: green }],
  unversioned: [],
  ...overrides,
});

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'credit-pme' });
  listMock.mockResolvedValue({ ok: true, data: { items: [first], total: 1 } });
});

it('lists the releases with where each one is live', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('viewer') });
  renderWithProviders(<ReleasesPage />);
  const table = await screen.findByRole('table', { name: 'Releases' });
  expect(await within(table).findByRole('button', { name: '1.0.0' })).toBeInTheDocument();
  expect(within(table).getByText('First scoring rules')).toBeInTheDocument();
  expect(within(table).getByText('Staging')).toBeInTheDocument();
  // Viewers read releases; they do not create them.
  expect(screen.queryByRole('button', { name: 'New release' })).not.toBeInTheDocument();
});

it('refuses a release while a decision has no version, naming it', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  previewMock.mockResolvedValue({ ok: true, data: preview({ unversioned: ['limits/max-amount'] }) });
  renderWithProviders(<ReleasesPage />);
  await user.click(await screen.findByRole('button', { name: 'New release' }));
  const dialog = await screen.findByRole('dialog', { name: 'New release' });
  expect(await within(dialog).findByText('Some decisions have no version')).toBeInTheDocument();
  expect(within(dialog).getByRole('link', { name: 'limits/max-amount' })).toHaveAttribute(
    'href',
    '/projects/decision?p=credit-pme&d=limits%2Fmax-amount',
  );
  expect(within(dialog).queryByRole('button', { name: /Create/ })).not.toBeInTheDocument();
});

it('creates a minor release with notes, warning about failing tests', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  previewMock.mockResolvedValue({
    ok: true,
    data: preview({
      decisions: [{ decisionId: 'd-1', key: 'bureau/normalize', version: 3, tests: { ...green, failed: 1 } }],
    }),
  });
  const created: Release = {
    ...first,
    id: 'r-2',
    version: '1.1.0',
    notes: 'Raise the SME ceiling',
    liveIn: [],
    decisions: [{ decisionId: 'd-1', key: 'bureau/normalize', version: 3, tests: green }],
  } as unknown as Release;
  createMock.mockResolvedValue({ ok: true, data: created });
  renderWithProviders(<ReleasesPage />);
  await user.click(await screen.findByRole('button', { name: 'New release' }));
  const dialog = await screen.findByRole('dialog', { name: 'New release' });
  expect(await within(dialog).findByText('Some tests fail')).toBeInTheDocument();
  expect(within(dialog).getByText('1 decision')).toBeInTheDocument();
  await user.type(within(dialog).getByLabelText(/Notes/), 'Raise the SME ceiling');
  await user.click(within(dialog).getByRole('button', { name: 'Create 1.1.0' }));
  expect(createMock).toHaveBeenCalledWith('p-1', { bump: 'minor', notes: 'Raise the SME ceiling' });
  // The toast offers the next step: deploying the new release to staging.
  expect(toast.success).toHaveBeenCalledWith(
    'Release 1.1.0 created.',
    expect.objectContaining({ action: expect.objectContaining({ label: 'Deploy to staging' }) }),
  );
});

it('asks for notes before creating a release', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('owner') });
  previewMock.mockResolvedValue({ ok: true, data: preview() });
  renderWithProviders(<ReleasesPage />);
  await user.click(await screen.findByRole('button', { name: 'New release' }));
  const dialog = await screen.findByRole('dialog', { name: 'New release' });
  await user.click(await within(dialog).findByRole('button', { name: 'Create 1.1.0' }));
  expect(await within(dialog).findByText('This field is required.')).toBeInTheDocument();
  expect(createMock).not.toHaveBeenCalled();
});

it('makes the first release 1.0.0 without asking for a bump', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('owner') });
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  previewMock.mockResolvedValue({
    ok: true,
    data: preview({ latest: undefined, next: { major: '1.0.0', minor: '1.0.0', patch: '1.0.0' } }),
  });
  renderWithProviders(<ReleasesPage />);
  const [create] = await screen.findAllByRole('button', { name: 'New release' });
  await user.click(create as HTMLElement);
  const dialog = await screen.findByRole('dialog', { name: 'New release' });
  expect(await within(dialog).findByText('A project’s first release is 1.0.0.')).toBeInTheDocument();
  expect(within(dialog).queryByRole('combobox')).not.toBeInTheDocument();
  expect(within(dialog).getByRole('button', { name: 'Create 1.0.0' })).toBeInTheDocument();
});
