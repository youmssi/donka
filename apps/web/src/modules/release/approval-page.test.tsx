import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { toast } from 'sonner';

import { getProjectByKey } from '@/modules/project/project.service';
import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { ApprovalPage } from './approval-page';
import { approve, getApproval, reject } from './release.service';
import type { ApprovalReview } from './schema';

vi.mock('./release.service', () => ({
  getApproval: vi.fn(),
  approve: vi.fn(),
  reject: vi.fn(),
  withdraw: vi.fn(),
}));
const getMock = vi.mocked(getApproval);
const approveMock = vi.mocked(approve);
const rejectMock = vi.mocked(reject);
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock('@/modules/project/project.service', () => ({ getProject: vi.fn(), getProjectByKey: vi.fn() }));
const projectMock = vi.mocked(getProjectByKey);
const alan = { id: 'u-2', email: 'alan@bank.example' };
const grace = { id: 'u-3', email: 'grace@bank.example' };
const me = { current: alan };
vi.mock('@/modules/identity', () => ({ useCurrentUser: () => me.current }));

const project = {
  id: 'p-1',
  key: 'credit-pme',
  name: 'Crédit PME',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role: 'owner' as const,
};
const review = (overrides: Partial<ApprovalReview> = {}): ApprovalReview => ({
  id: 'a-1',
  releaseId: 'r-2',
  releaseVersion: '1.1.0',
  releaseNotes: 'Score v2 for SMEs',
  releaseCreatedBy: grace,
  requestedBy: grace,
  requestedAt: '2026-10-01T09:00:00Z',
  status: 'pending',
  productionVersion: '1.0.0',
  changes: [
    {
      key: 'person-score',
      decisionId: 'd-2',
      change: 'changed',
      fromVersion: 1,
      toVersion: 2,
      tests: { passed: 1, failed: 1, errors: 0 },
    },
    {
      key: 'bureau/normalize',
      decisionId: 'd-1',
      change: 'unchanged',
      fromVersion: 1,
      toVersion: 1,
      tests: { passed: 2, failed: 0, errors: 0 },
    },
  ],
  tests: { passed: 3, failed: 1, errors: 0 },
  canDecide: true,
  ...overrides,
});

beforeEach(() => {
  vi.clearAllMocks();
  me.current = alan;
  search.params = new URLSearchParams({ p: 'credit-pme', a: 'a-1' });
  projectMock.mockResolvedValue({ ok: true, data: project });
});

it('shows what changes in production, the tests and the notes', async () => {
  getMock.mockResolvedValue({ ok: true, data: review() });
  renderWithProviders(<ApprovalPage />);
  expect(await screen.findByRole('heading', { name: 'Release 1.1.0 to production' })).toBeInTheDocument();
  expect(screen.getByText('1 decision changes compared with production.')).toBeInTheDocument();
  expect(screen.getByText('Changed')).toBeInTheDocument();
  expect(screen.getByText('v1 → v2')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Compare person-score' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Compare bureau/normalize' })).not.toBeInTheDocument();
  expect(screen.getByText('Score v2 for SMEs')).toBeInTheDocument();
  expect(screen.getByText(/Some scenarios fail/)).toBeInTheDocument();
});

it('lets an owner who did not make the release approve it', async () => {
  const user = userEvent.setup();
  getMock.mockResolvedValue({ ok: true, data: review() });
  approveMock.mockResolvedValue({ ok: true, data: { ...review(), status: 'approved' } });
  renderWithProviders(<ApprovalPage />);
  await user.click(await screen.findByRole('button', { name: 'Approve' }));
  const confirm = await screen.findByRole('alertdialog', { name: 'Approve release 1.1.0 for production?' });
  await user.click(within(confirm).getByRole('button', { name: 'Approve' }));
  expect(approveMock).toHaveBeenCalledWith('p-1', 'a-1');
  expect(toast.success).toHaveBeenCalledWith('Release 1.1.0 approved: publishing to production.');
  expect(screen.queryByText(/{version}/)).not.toBeInTheDocument();
});

it('asks for a reason to reject', async () => {
  const user = userEvent.setup();
  getMock.mockResolvedValue({ ok: true, data: review() });
  rejectMock.mockResolvedValue({ ok: true, data: { ...review(), status: 'rejected', reason: 'Needs risk sign-off' } });
  renderWithProviders(<ApprovalPage />);
  await user.click(await screen.findByRole('button', { name: 'Reject' }));
  const dialog = await screen.findByRole('dialog', { name: 'Reject release 1.1.0?' });
  await user.click(within(dialog).getByRole('button', { name: 'Reject' }));
  expect(await within(dialog).findByText('This field is required.')).toBeInTheDocument();
  expect(rejectMock).not.toHaveBeenCalled();
  await user.type(within(dialog).getByLabelText(/Reason/), 'Needs risk sign-off');
  await user.click(within(dialog).getByRole('button', { name: 'Reject' }));
  expect(rejectMock).toHaveBeenCalledWith('p-1', 'a-1', 'Needs risk sign-off');
});

it('tells the author that another owner decides', async () => {
  me.current = grace;
  getMock.mockResolvedValue({ ok: true, data: review({ canDecide: false }) });
  renderWithProviders(<ApprovalPage />);
  expect(await screen.findByText('You made this release or asked for it: another owner decides.')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Withdraw' })).toBeInTheDocument();
});

it('shows why a request was rejected', async () => {
  getMock.mockResolvedValue({
    ok: true,
    data: review({
      status: 'rejected',
      canDecide: false,
      decidedBy: alan,
      decidedAt: '2026-10-01T10:00:00Z',
      reason: 'Needs risk sign-off',
    }),
  });
  renderWithProviders(<ApprovalPage />);
  expect(await screen.findByText('Rejected by alan@bank.example')).toBeInTheDocument();
  // The status badge names the status, not a sentence about it.
  expect(screen.getByText('Rejected', { exact: true })).toBeInTheDocument();
  expect(screen.getByText('Needs risk sign-off')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Reject' })).not.toBeInTheDocument();
});
