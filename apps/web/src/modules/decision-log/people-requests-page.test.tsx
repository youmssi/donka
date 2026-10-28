import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { toast } from 'sonner';

import { getProjectByKey } from '@/modules/project/project.service';
import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { anonymisePerson, findPerson, getSettings } from './decision-log.service';
import { PeopleRequestsPage } from './people-requests-page';
import type { PersonRecord } from './schema';

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

vi.mock('./decision-log.service', async (original) => ({
  ...(await original<typeof import('./decision-log.service')>()),
  findPerson: vi.fn(),
  anonymisePerson: vi.fn(),
  getSettings: vi.fn(),
}));
const findMock = vi.mocked(findPerson);
const anonymiseMock = vi.mocked(anonymisePerson);

vi.mock('@/modules/project/project.service', () => ({
  getProject: vi.fn(),
  getProjectByKey: vi.fn(),
  listMembers: vi.fn(),
  listProjects: vi.fn(),
}));

function as(role: 'owner' | 'viewer') {
  vi.mocked(getProjectByKey).mockResolvedValue({
    ok: true,
    data: {
      id: 'p-1',
      key: 'retail',
      name: 'Retail',
      description: '',
      createdAt: '2026-09-28T09:00:00Z',
      archivedAt: null,
      role,
    },
  });
}

const found: PersonRecord[] = [
  {
    id: 'r-2',
    decisionKey: 'limit',
    reference: 'APP-2026-0043',
    environment: 'production',
    status: 'succeeded',
    outcome: 'decline',
    releaseId: 'rel-1',
    evaluatedAt: '2026-10-02T10:00:00Z',
    durationUs: 840,
    anonymisedAt: null,
  },
  {
    id: 'r-1',
    decisionKey: 'limit',
    reference: 'APP-2026-0042',
    environment: 'production',
    status: 'succeeded',
    outcome: 'approve',
    releaseId: 'rel-1',
    evaluatedAt: '2026-08-30T10:00:00Z',
    durationUs: 640,
    anonymisedAt: null,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'retail' });
  as('owner');
  vi.mocked(getSettings).mockResolvedValue({
    ok: true,
    data: { outcomeField: 'decision', redactedFields: [], explainEnabled: false },
  });
  findMock.mockResolvedValue({ ok: true, data: found });
});

async function searchFor(identifier: string) {
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Identifier'), identifier);
  await user.click(screen.getByRole('button', { name: 'Search' }));
  return user;
}

it('finds the decisions about a person by an identifier, kept out of the address', async () => {
  renderWithProviders(<PeopleRequestsPage />);
  await searchFor(' CM-1984-0042 ');
  expect(await screen.findByText('2 decisions found')).toBeInTheDocument();
  expect(findMock).toHaveBeenCalledWith('p-1', 'CM-1984-0042');
  const table = screen.getByRole('table', { name: 'Decisions about the person' });
  expect(within(table).getByRole('link', { name: /APP-2026-0042/ })).toHaveAttribute(
    'href',
    '/projects/decision-record?p=retail&r=r-1',
  );
  expect(search.params.toString()).toBe('p=retail');
});

it('says when nothing holds the identifier', async () => {
  findMock.mockResolvedValue({ ok: true, data: [] });
  renderWithProviders(<PeopleRequestsPage />);
  await searchFor('CM-0000');
  expect(await screen.findByText('No decision found')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Anonymise' })).not.toBeInTheDocument();
});

it('shows the retention rule that blocks anonymisation', async () => {
  anonymiseMock.mockResolvedValue({
    ok: false,
    error: {
      code: 'RETENTION_RULE_RUNS',
      details: {
        rule: { description: 'Loan dispute with the applicant', legalReference: 'Case 118', until: '2026-12-31' },
      },
    },
  });
  renderWithProviders(<PeopleRequestsPage />);
  const user = await searchFor('CM-1984-0042');
  await user.click(await screen.findByRole('button', { name: 'Anonymise' }));
  expect(screen.getByText(/removed from 2 decision records/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Anonymise for good' }));
  expect(await screen.findByText('Loan dispute with the applicant')).toBeInTheDocument();
  expect(screen.getByText(/A retention rule runs until 31 Dec/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Anonymise for good' })).not.toBeInTheDocument();
  expect(anonymiseMock).toHaveBeenCalledWith('p-1', 'CM-1984-0042');
});

it('anonymises, then searches again', async () => {
  anonymiseMock.mockResolvedValue({ ok: true, data: 2 });
  renderWithProviders(<PeopleRequestsPage />);
  const user = await searchFor('CM-1984-0042');
  findMock.mockResolvedValue({ ok: true, data: [] });
  await user.click(await screen.findByRole('button', { name: 'Anonymise' }));
  await user.click(screen.getByRole('button', { name: 'Anonymise for good' }));
  expect(await screen.findByText('No decision found')).toBeInTheDocument();
  expect(toast.success).toHaveBeenCalledWith('2 records anonymised.');
  expect(findMock).toHaveBeenCalledTimes(2);
});

it('is for owners only', async () => {
  as('viewer');
  renderWithProviders(<PeopleRequestsPage />);
  expect(
    await screen.findByText("Only the project's owners handle the requests of the people decisions are about."),
  ).toBeInTheDocument();
  expect(screen.queryByLabelText('Identifier')).not.toBeInTheDocument();
});
