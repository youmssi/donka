import { screen, within } from '@testing-library/react';

import { getProjectByKey } from '@/modules/project/project.service';
import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { listDecisions } from './decision.service';
import { DecisionsPage } from './decisions-page';

vi.mock('./decision.service', () => ({ listDecisions: vi.fn(), createDecision: vi.fn(), deleteDecision: vi.fn() }));
const listMock = vi.mocked(listDecisions);
vi.mock('@/modules/project/project.service', () => ({ getProject: vi.fn(), getProjectByKey: vi.fn() }));
const projectMock = vi.mocked(getProjectByKey);

const project = (role: 'owner' | 'editor' | 'viewer', archivedAt: string | null = null) => ({
  id: 'p-1',
  key: 'credit-pme',
  name: 'Crédit PME',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt,
  role,
});
const decisions = [
  {
    id: 'd-1',
    key: 'bureau/normalize',
    revision: 2,
    updatedAt: '2026-09-29T09:00:00Z',
    updatedBy: { id: 'u-1', email: 'ada@bank.example' },
    latestVersion: 2,
    changedSinceVersion: false,
  },
  {
    id: 'd-2',
    key: 'person-score',
    revision: 1,
    updatedAt: '2026-09-29T09:00:00Z',
    updatedBy: { id: 'u-2', email: 'grace@bank.example' },
    latestVersion: null,
    changedSinceVersion: true,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'credit-pme' });
  listMock.mockResolvedValue({ ok: true, data: decisions });
});

it('lists the decisions with a link to each editor', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  renderWithProviders(<DecisionsPage />);
  const table = await screen.findByRole('table', { name: 'Decisions' });
  const link = await within(table).findByRole('link', { name: 'bureau/normalize' });
  expect(link).toHaveAttribute('href', '/projects/decision?p=credit-pme&d=bureau%2Fnormalize');
  expect(within(table).getByText('grace@bank.example')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'New decision' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Actions for person-score' })).toBeInTheDocument();
});

it('lets a viewer open decisions but not create or delete them', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('viewer') });
  renderWithProviders(<DecisionsPage />);
  await screen.findByRole('link', { name: 'person-score' });
  expect(screen.queryByRole('button', { name: 'New decision' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Actions for/ })).not.toBeInTheDocument();
});

it('keeps an archived project read-only, and says how to start in an empty one', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('owner', '2026-09-29T10:00:00Z') });
  const view = renderWithProviders(<DecisionsPage />);
  await screen.findByRole('link', { name: 'person-score' });
  expect(screen.queryByRole('button', { name: 'New decision' })).not.toBeInTheDocument();
  view.unmount();

  projectMock.mockResolvedValue({ ok: true, data: project('owner') });
  listMock.mockResolvedValue({ ok: true, data: [] });
  renderWithProviders(<DecisionsPage />);
  expect(await screen.findByText('No decisions yet')).toBeInTheDocument();
  expect(screen.getAllByRole('button', { name: 'New decision' }).length).toBeGreaterThan(0);
});
