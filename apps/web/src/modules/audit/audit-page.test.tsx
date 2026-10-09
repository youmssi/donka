import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';
import { getProjectByKey, listMembers } from '@/modules/project/project.service';

import { listAudit } from './audit.service';
import { ProjectAuditPage } from './audit-page';
import type { AuditEvent } from './schema';

vi.mock('./audit.service', async (original) => ({
  ...(await original<typeof import('./audit.service')>()),
  listAudit: vi.fn(),
}));
const listAuditMock = vi.mocked(listAudit);

vi.mock('@/modules/project/project.service', () => ({
  getProject: vi.fn(),
  getProjectByKey: vi.fn(),
  listMembers: vi.fn(),
  listProjects: vi.fn(),
}));
const getProjectMock = vi.mocked(getProjectByKey);
vi.mocked(listMembers).mockResolvedValue({
  ok: true,
  data: [{ userId: 'u-2', email: 'grace@bank.example', role: 'owner', addedAt: '2026-09-28T09:00:00Z' }],
});

const project = (role: 'owner' | 'editor') => ({
  id: 'p-1',
  key: 'retail',
  name: 'Retail scoring',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});
const ada = { id: 'u-1', email: 'ada@bank.example' };
const grace = { id: 'u-2', email: 'grace@bank.example' };
const events: AuditEvent[] = [
  {
    id: 5,
    occurredAt: '2026-09-29T11:00:00Z',
    actor: ada,
    action: 'decision.version_restored',
    details: { key: 'person-score', version: 3, from: 1 },
  },
  {
    id: 4,
    occurredAt: '2026-09-29T10:30:00Z',
    actor: ada,
    action: 'decision.version_saved',
    details: { key: 'person-score', version: 2, message: 'Raise the threshold' },
  },
  {
    id: 3,
    occurredAt: '2026-09-29T10:00:00Z',
    actor: grace,
    action: 'member.role_changed',
    target: ada,
    details: { from: 'owner', to: 'editor' },
  },
  {
    id: 2,
    occurredAt: '2026-09-28T10:00:00Z',
    actor: ada,
    action: 'project.updated',
    details: { from: { name: 'Retail', description: '' }, to: { name: 'Retail scoring', description: '' } },
  },
  {
    id: 1,
    occurredAt: '2026-09-28T09:00:00Z',
    actor: ada,
    action: 'project.created',
    details: { key: 'retail', name: 'Retail' },
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'retail' });
  getProjectMock.mockResolvedValue({ ok: true, data: project('owner') });
  listAuditMock.mockResolvedValue({ ok: true, data: { items: events, total: 5 } });
});

it('tells an owner what changed, who did it and when', async () => {
  renderWithProviders(<ProjectAuditPage />);
  expect(await screen.findByText('Changed the role of ada@bank.example from Owner to Editor')).toBeInTheDocument();
  expect(screen.getByText('Renamed the project from “Retail” to “Retail scoring”')).toBeInTheDocument();
  expect(screen.getByText('Created the project “Retail”')).toBeInTheDocument();
  expect(screen.getByText('Saved version 2 of person-score: “Raise the threshold”')).toBeInTheDocument();
  expect(screen.getByText('Restored version 1 of person-score as version 3')).toBeInTheDocument();
  expect(screen.getByText('by grace@bank.example')).toBeInTheDocument();
  expect(screen.getByRole('heading', { name: 'Audit log' })).toBeInTheDocument();
  expect(screen.getByText(/^Times in /)).toBeInTheDocument();
  // Short on screen, the full date and time on hover; the element keeps the exact instant.
  expect(document.querySelector('time')).toHaveAttribute('dateTime', '2026-09-29T11:00:00Z');
});

it('says what Studio did on its own, such as re-sealing records after a key rotation', async () => {
  const resealed: AuditEvent = {
    id: 6,
    occurredAt: '2026-10-09T08:00:00Z',
    actor: null,
    action: 'decision_log.resealed',
    details: { records: 500, fromKeys: ['a1b2c3d4e5f60718'], toKey: '0f1e2d3c4b5a6978' },
  };
  listAuditMock.mockResolvedValue({ ok: true, data: { items: [resealed], total: 1 } });
  renderWithProviders(<ProjectAuditPage />);
  expect(await screen.findByText('Re-sealed 500 decision records with the current key')).toBeInTheDocument();
  expect(screen.getByRole('cell', { name: 'Studio' })).toBeInTheDocument();
});

it('filters from the address and exports the same events', async () => {
  search.params = new URLSearchParams({ p: 'retail', actor: 'u-2', action: 'member.added', from: '2026-09-01' });
  renderWithProviders(<ProjectAuditPage />);
  await screen.findByText('Created the project “Retail”');
  const query = { actor: 'u-2', action: 'member.added', from: new Date(2026, 8, 1).toISOString(), until: undefined };
  expect(listAuditMock).toHaveBeenCalledWith('p-1', query, 0);
  const href = screen.getByRole('link', { name: 'Export CSV' }).getAttribute('href') ?? '';
  const url = new URL(href, 'http://studio.test');
  expect(url.pathname).toBe('/api/v1/projects/p-1/audit/export');
  expect(Object.fromEntries(url.searchParams)).toEqual({ actor: 'u-2', action: 'member.added', from: query.from });
  expect(screen.getAllByRole('link', { name: 'Clear filters' })[0]).toHaveAttribute('href', '/projects/audit?p=retail');
  expect(screen.getByRole('combobox', { name: 'Person' })).toHaveTextContent('grace@bank.example');
});

it('changes a filter through the address and starts from the first page', async () => {
  search.params = new URLSearchParams({ p: 'retail', offset: '50' });
  renderWithProviders(<ProjectAuditPage />);
  await screen.findByText('Created the project “Retail”');
  const user = userEvent.setup();
  await user.click(screen.getByRole('combobox', { name: 'Person' }));
  await user.click(await screen.findByRole('option', { name: 'grace@bank.example' }));
  expect(router.replace).toHaveBeenLastCalledWith('/projects/audit?p=retail&actor=u-2');
});

it('says when nothing matches, and when the dates cannot match', async () => {
  listAuditMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  search.params = new URLSearchParams({ p: 'retail', action: 'member.removed' });
  const view = renderWithProviders(<ProjectAuditPage />);
  expect(await screen.findByText('No change matches these filters.')).toBeInTheDocument();
  view.unmount();

  listAuditMock.mockClear();
  search.params = new URLSearchParams({ p: 'retail', from: '2026-09-30', to: '2026-09-01' });
  renderWithProviders(<ProjectAuditPage />);
  expect(await screen.findByText('The end date is before the start date.')).toBeInTheDocument();
  expect(listAuditMock).not.toHaveBeenCalled();
});

it('shows a failure with a way to try again', async () => {
  listAuditMock.mockResolvedValue({ ok: false, error: { code: 'NETWORK' } });
  renderWithProviders(<ProjectAuditPage />);
  expect(await screen.findByText('We could not load the audit log')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
});

it('keeps the log and its tab from anyone but owners', async () => {
  getProjectMock.mockResolvedValue({ ok: true, data: project('editor') });
  renderWithProviders(<ProjectAuditPage />);
  expect(await screen.findByText("Only the project's owners can see its audit log.")).toBeInTheDocument();
  expect(screen.queryByRole('link', { name: 'Export CSV' })).not.toBeInTheDocument();
  expect(listAuditMock).not.toHaveBeenCalled();
});
