import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { getProjectByKey } from '@/modules/project/project.service';
import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { searchRecords } from './decision-log.service';
import { DecisionLogPage } from './decision-log-page';
import type { RecordSummary } from './schema';

vi.mock('./decision-log.service', async (original) => ({
  ...(await original<typeof import('./decision-log.service')>()),
  searchRecords: vi.fn(),
}));
const searchMock = vi.mocked(searchRecords);

vi.mock('@/modules/project/project.service', () => ({
  getProject: vi.fn(),
  getProjectByKey: vi.fn(),
  listMembers: vi.fn(),
  listProjects: vi.fn(),
}));

const records: RecordSummary[] = [
  {
    id: 'r-2',
    decisionKey: 'person-score',
    reference: 'APP-2026-0043',
    environment: 'production',
    status: 'failed',
    outcome: 'error',
    releaseId: 'rel-1',
    evaluatedAt: '2026-10-02T10:01:00Z',
    durationUs: 1840,
  },
  {
    id: 'r-1',
    decisionKey: 'person-score',
    reference: null,
    environment: 'staging',
    status: 'succeeded',
    outcome: 'approve',
    releaseId: 'rel-1',
    evaluatedAt: '2026-10-02T10:00:00Z',
    durationUs: 420,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'retail' });
  vi.mocked(getProjectByKey).mockResolvedValue({
    ok: true,
    data: {
      id: 'p-1',
      key: 'retail',
      name: 'Retail',
      description: '',
      createdAt: '2026-09-28T09:00:00Z',
      archivedAt: null,
      role: 'viewer',
    },
  });
  searchMock.mockResolvedValue({ ok: true, data: { items: records, total: 2 } });
});

it('lists the decisions the Runtimes made, newest first, to any member', async () => {
  renderWithProviders(<DecisionLogPage />);
  expect(await screen.findByText('APP-2026-0043')).toBeInTheDocument();
  expect(screen.getByRole('heading', { name: 'Decision log' })).toBeInTheDocument();
  expect(screen.getByText('approve')).toBeInTheDocument();
  expect(screen.getByText('Failed')).toBeInTheDocument();
  expect(screen.getByText('No reference')).toBeInTheDocument();
  expect(screen.getByText('420 µs')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: /APP-2026-0043/ })).toHaveAttribute(
    'href',
    '/projects/decision-record?p=retail&r=r-2',
  );
  expect(searchMock).toHaveBeenCalledWith('p-1', expect.objectContaining({ reference: undefined }), 0);
});

it('searches by reference, decision and outcome through the address', async () => {
  search.params = new URLSearchParams({ p: 'retail', offset: '50' });
  renderWithProviders(<DecisionLogPage />);
  await screen.findByText('APP-2026-0043');
  const user = userEvent.setup();
  await user.type(screen.getByRole('textbox', { name: 'Reference' }), 'APP-2026-0043');
  await user.type(screen.getByRole('textbox', { name: 'Outcome' }), 'approve{Enter}');
  expect(router.replace).toHaveBeenLastCalledWith(
    '/projects/decision-log?p=retail&reference=APP-2026-0043&outcome=approve',
  );
});

it('filters from the address and says when nothing matches', async () => {
  searchMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  search.params = new URLSearchParams({ p: 'retail', status: 'failed', environment: 'production' });
  renderWithProviders(<DecisionLogPage />);
  expect(await screen.findByText('No decision matches')).toBeInTheDocument();
  expect(searchMock).toHaveBeenCalledWith(
    'p-1',
    expect.objectContaining({ status: 'failed', environment: 'production' }),
    0,
  );
  expect(screen.getAllByRole('link', { name: 'Clear filters' })[0]).toHaveAttribute(
    'href',
    '/projects/decision-log?p=retail',
  );
});

it('explains how decisions arrive when there are none yet', async () => {
  searchMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<DecisionLogPage />);
  expect(await screen.findByText('No decision yet')).toBeInTheDocument();
  expect(screen.getByText(/issues a decision-log token under Runtimes/)).toBeInTheDocument();
});

it('shows a failure with a way to try again', async () => {
  searchMock.mockResolvedValue({ ok: false, error: { code: 'NETWORK' } });
  renderWithProviders(<DecisionLogPage />);
  expect(await screen.findByText('The decision log could not be loaded.')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
});
