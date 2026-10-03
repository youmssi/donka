import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { getProjectByKey } from '@/modules/project/project.service';
import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { explainRecord, getRecord, replayRecord } from './decision-log.service';
import { RecordPage } from './record-page';
import type { DecisionRecord } from './schema';

vi.mock('./decision-log.service', async (original) => ({
  ...(await original<typeof import('./decision-log.service')>()),
  getRecord: vi.fn(),
  replayRecord: vi.fn(),
  explainRecord: vi.fn(),
}));
const getMock = vi.mocked(getRecord);
const replayMock = vi.mocked(replayRecord);
const explainMock = vi.mocked(explainRecord);

vi.mock('@/modules/project/project.service', () => ({
  getProject: vi.fn(),
  getProjectByKey: vi.fn(),
  listMembers: vi.fn(),
  listProjects: vi.fn(),
}));

const record: DecisionRecord = {
  id: 'r-1',
  decisionKey: 'person-score',
  reference: 'APP-2026-0042',
  environment: 'production',
  status: 'succeeded',
  outcome: 'approve',
  releaseId: 'rel-1',
  releaseVersion: '1.4.0',
  evaluatedAt: '2026-10-02T10:00:00Z',
  receivedAt: '2026-10-02T10:00:01Z',
  durationUs: 840,
  input: { applicant: { nationalId: 'CM-1984-0042' } },
  output: { decision: 'approve' },
  error: null,
  trace: { score: { output: { decision: 'approve' } } },
  explainable: false,
};

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'retail', r: 'r-1' });
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
  getMock.mockResolvedValue({ ok: true, data: record });
});

it('shows what the decision was asked and answered, and that opening it is audited', async () => {
  renderWithProviders(<RecordPage />);
  expect(await screen.findByRole('heading', { name: 'APP-2026-0042' })).toBeInTheDocument();
  expect(screen.getByText(/CM-1984-0042/)).toBeInTheDocument();
  expect(screen.getByText('1.4.0')).toBeInTheDocument();
  expect(screen.getByText(/recorded in the project's audit log/)).toBeInTheDocument();
  expect(getMock).toHaveBeenCalledTimes(1);
  expect(getMock).toHaveBeenCalledWith('p-1', 'r-1');
});

it('replays the decision and says the result is the same', async () => {
  replayMock.mockResolvedValue({
    ok: true,
    data: { identical: true, status: 'succeeded', output: { decision: 'approve' }, error: null },
  });
  renderWithProviders(<RecordPage />);
  await screen.findByRole('heading', { name: 'APP-2026-0042' });
  await userEvent.setup().click(screen.getByRole('button', { name: 'Replay' }));
  expect(await screen.findByText('Same result')).toBeInTheDocument();
  expect(screen.getByText('Release 1.4.0 gives the same answer again.')).toBeInTheDocument();
  expect(replayMock).toHaveBeenCalledWith('p-1', 'r-1');
});

it('shows both answers when a replay differs', async () => {
  replayMock.mockResolvedValue({
    ok: true,
    data: { identical: false, status: 'succeeded', output: { decision: 'decline' }, error: null },
  });
  renderWithProviders(<RecordPage />);
  await screen.findByRole('heading', { name: 'APP-2026-0042' });
  await userEvent.setup().click(screen.getByRole('button', { name: 'Replay' }));
  expect(await screen.findByText('Different result')).toBeInTheDocument();
  expect(screen.getByText('Recorded answer')).toBeInTheDocument();
  expect(screen.getByText(/"decline"/)).toBeInTheDocument();
});

it('says when the record does not exist', async () => {
  getMock.mockResolvedValue({ ok: false, error: { code: 'RECORD_NOT_FOUND' } });
  renderWithProviders(<RecordPage />);
  expect(await screen.findByText('Decision record not found')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Back to the decision log' })).toHaveAttribute(
    'href',
    '/projects/decision-log?p=retail',
  );
});

it('offers no explanation when the installation does not explain decisions', async () => {
  renderWithProviders(<RecordPage />);
  await screen.findByRole('heading', { name: 'APP-2026-0042' });
  expect(screen.queryByRole('button', { name: 'Explain' })).not.toBeInTheDocument();
});

it('explains the decision in the reader’s language, as text only', async () => {
  getMock.mockResolvedValue({ ok: true, data: { ...record, explainable: true } });
  explainMock.mockResolvedValue({
    ok: true,
    data: { explanation: 'The bureau score was above 700, so the table approved.', model: 'bank-llm' },
  });
  renderWithProviders(<RecordPage />);
  await screen.findByRole('heading', { name: 'APP-2026-0042' });
  await userEvent.setup().click(screen.getByRole('button', { name: 'Explain' }));
  expect(await screen.findByText('The bureau score was above 700, so the table approved.')).toBeInTheDocument();
  expect(
    screen.getByText(/Written by bank-llm from this record, without the project's redacted fields/),
  ).toBeInTheDocument();
  expect(explainMock).toHaveBeenCalledWith('p-1', 'r-1', 'en');
});

it('says when no explanation could be had', async () => {
  getMock.mockResolvedValue({ ok: true, data: { ...record, explainable: true } });
  explainMock.mockResolvedValue({ ok: false, error: { code: 'EXPLAIN_UNAVAILABLE' } });
  renderWithProviders(<RecordPage />);
  await screen.findByRole('heading', { name: 'APP-2026-0042' });
  await userEvent.setup().click(screen.getByRole('button', { name: 'Explain' }));
  expect(await screen.findByText('No explanation this time.')).toBeInTheDocument();
  expect(screen.getByText('The explanation service did not answer. Try again in a moment.')).toBeInTheDocument();
});
