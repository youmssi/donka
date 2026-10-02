import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { getProjectByKey } from '@/modules/project/project.service';
import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { EnvironmentsPage } from './environments-page';
import {
  issueToken,
  listApprovals,
  listEnvironments,
  listReleases,
  listTokens,
  requestApproval,
  retryDeployment,
} from './release.service';
import type { Deployment, EnvironmentState } from './schema';

vi.mock('./release.service', () => ({
  RELEASES_PAGE_SIZE: 50,
  listReleases: vi.fn(),
  listEnvironments: vi.fn(),
  listTokens: vi.fn(),
  deployRelease: vi.fn(),
  retryDeployment: vi.fn(),
  issueToken: vi.fn(),
  revokeToken: vi.fn(),
  listApprovals: vi.fn(),
  requestApproval: vi.fn(),
}));
const approvalsMock = vi.mocked(listApprovals);
const requestMock = vi.mocked(requestApproval);
const environmentsMock = vi.mocked(listEnvironments);
const releasesMock = vi.mocked(listReleases);
const tokensMock = vi.mocked(listTokens);
const retryMock = vi.mocked(retryDeployment);
const issueMock = vi.mocked(issueToken);
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
const deployment = (overrides: Partial<Deployment>): Deployment => ({
  id: 'dep-1',
  environment: 'staging',
  releaseId: 'r-1',
  releaseVersion: '1.0.0',
  reason: 'deploy',
  requestedAt: '2026-09-30T09:00:00Z',
  requestedBy: ada,
  status: 'published',
  attempts: 1,
  publishedAt: '2026-09-30T09:00:05Z',
  ...overrides,
});
const environments = (staging: Partial<EnvironmentState> = {}): EnvironmentState[] => [
  { environment: 'staging', tokens: 0, ...staging },
  { environment: 'production', tokens: 0 },
];

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'credit-pme' });
  releasesMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  tokensMock.mockResolvedValue({ ok: true, data: [] });
  approvalsMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
});

it('shows the live release of each environment and its object key', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('viewer') });
  const live = deployment({});
  environmentsMock.mockResolvedValue({ ok: true, data: environments({ live, latest: live }) });
  renderWithProviders(<EnvironmentsPage />);
  const staging = await screen.findByRole('region', { name: 'Staging' });
  expect(await within(staging).findByText('1.0.0')).toBeInTheDocument();
  expect(within(staging).getByText('staging/credit-pme')).toBeInTheDocument();
  const production = screen.getByRole('region', { name: 'Production' });
  expect(within(production).getByText('No release deployed yet.')).toBeInTheDocument();
  // Viewers see where things stand, without the controls.
  expect(screen.queryByRole('button', { name: 'Deploy' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'New token' })).not.toBeInTheDocument();
});

it('says a deployment gave up, with why, and retries it', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  const failed = deployment({
    id: 'dep-2',
    releaseVersion: '1.1.0',
    status: 'failed',
    attempts: 10,
    publishedAt: undefined,
    lastError: 'bucket unreachable',
  });
  environmentsMock.mockResolvedValue({ ok: true, data: environments({ latest: failed }) });
  retryMock.mockResolvedValue({ ok: true, data: { ...failed, status: 'pending', attempts: 0 } });
  renderWithProviders(<EnvironmentsPage />);
  const staging = await screen.findByRole('region', { name: 'Staging' });
  expect(await within(staging).findByText('Deploying 1.1.0 failed')).toBeInTheDocument();
  expect(within(staging).getByText('Gave up after 10 attempts: bucket unreachable')).toBeInTheDocument();
  await user.click(within(staging).getByRole('button', { name: 'Retry' }));
  expect(retryMock).toHaveBeenCalledWith('p-1', 'staging', 'dep-2');
});

it('shows a new token once, with how to send it', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('owner') });
  environmentsMock.mockResolvedValue({ ok: true, data: environments() });
  issueMock.mockResolvedValue({
    ok: true,
    data: {
      id: 't-1',
      name: 'Loan origination',
      hint: 'abcd',
      createdAt: '2026-10-01T09:00:00Z',
      createdBy: ada,
      token: 'dnk_secret-value-abcd',
    },
  });
  renderWithProviders(<EnvironmentsPage />);
  const staging = await screen.findByRole('region', { name: 'Staging' });
  await user.click(await within(staging).findByRole('button', { name: 'New token' }));
  const dialog = await screen.findByRole('dialog', { name: 'New Staging token' });
  await user.type(within(dialog).getByLabelText(/Name/), 'Loan origination');
  await user.click(within(dialog).getByRole('button', { name: 'New token' }));
  expect(issueMock).toHaveBeenCalledWith('p-1', 'staging', 'Loan origination');
  expect(await within(dialog).findByDisplayValue('dnk_secret-value-abcd')).toBeInTheDocument();
  expect(within(dialog).getByText(/shown only once/)).toBeInTheDocument();
  expect(within(dialog).getByText(/X-Access-Token/)).toBeInTheDocument();
});

it('asks for the release live on staging to go to production', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  const live = deployment({ releaseId: 'r-2', releaseVersion: '1.1.0' });
  environmentsMock.mockResolvedValue({ ok: true, data: environments({ live, latest: live }) });
  requestMock.mockResolvedValue({
    ok: true,
    data: {
      id: 'a-1',
      releaseId: 'r-2',
      releaseVersion: '1.1.0',
      releaseNotes: 'Raise the ceiling',
      releaseCreatedBy: ada,
      requestedBy: ada,
      requestedAt: '2026-10-01T09:00:00Z',
      status: 'pending',
    },
  });
  renderWithProviders(<EnvironmentsPage />);
  const production = await screen.findByRole('region', { name: 'Production' });
  await user.click(await within(production).findByRole('button', { name: 'Ask to publish 1.1.0' }));
  await user.click(await screen.findByRole('button', { name: 'Ask for approval' }));
  expect(requestMock).toHaveBeenCalledWith('p-1', 'r-2');
  expect(router.push).toHaveBeenCalledWith('/projects/approval?p=credit-pme&a=a-1');
});

it('shows the request waiting for approval instead of asking again', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  const live = deployment({ releaseId: 'r-2', releaseVersion: '1.1.0' });
  environmentsMock.mockResolvedValue({ ok: true, data: environments({ live, latest: live }) });
  approvalsMock.mockResolvedValue({
    ok: true,
    data: {
      items: [
        {
          id: 'a-1',
          releaseId: 'r-2',
          releaseVersion: '1.1.0',
          releaseNotes: 'Raise the ceiling',
          releaseCreatedBy: ada,
          requestedBy: ada,
          requestedAt: '2026-10-01T09:00:00Z',
          status: 'pending',
        },
      ],
      total: 1,
    },
  });
  renderWithProviders(<EnvironmentsPage />);
  const production = await screen.findByRole('region', { name: 'Production' });
  expect(await within(production).findByText(/1.1.0 waits for approval/)).toBeInTheDocument();
  expect(within(production).getByRole('link', { name: 'Review' })).toHaveAttribute(
    'href',
    '/projects/approval?p=credit-pme&a=a-1',
  );
  expect(within(production).queryByRole('button', { name: /Ask to publish/ })).not.toBeInTheDocument();
});
