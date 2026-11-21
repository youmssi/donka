import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { createRetentionRule, endRetentionRule, listRetentionRules } from './decision-log.service';
import { RetentionRules } from './retention-rules';
import { localToday, type RetentionRule } from './schema';

vi.mock('./decision-log.service', async (original) => ({
  ...(await original<typeof import('./decision-log.service')>()),
  listRetentionRules: vi.fn(),
  createRetentionRule: vi.fn(),
  endRetentionRule: vi.fn(),
}));
const listMock = vi.mocked(listRetentionRules);
const createMock = vi.mocked(createRetentionRule);
const endMock = vi.mocked(endRetentionRule);

const project = (role: 'owner' | 'viewer') => ({
  id: 'p-1',
  key: 'retail',
  name: 'Retail',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});

const rule: RetentionRule = {
  id: 'rule-1',
  description: 'Loan dispute with the applicant',
  legalReference: 'Court case 2026/118',
  until: '2099-12-31',
  createdBy: 'u-1',
  createdAt: '2026-10-01T09:00:00Z',
  endedBy: null,
  endedAt: null,
  running: true,
};

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockResolvedValue({ ok: true, data: [rule] });
});

it('lists the rules; viewers change nothing', async () => {
  renderWithProviders(<RetentionRules project={project('viewer')} />);
  expect(await screen.findByText('Loan dispute with the applicant')).toBeInTheDocument();
  expect(screen.getByText('Running')).toBeInTheDocument();
  expect(screen.getByText('Court case 2026/118')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'End the rule' })).not.toBeInTheDocument();
  expect(screen.getByText('Only owners document and end retention rules.')).toBeInTheDocument();
});

it('lets owners document a rule that ends today or later', async () => {
  createMock.mockResolvedValue({ ok: true, data: { ...rule, id: 'rule-2' } });
  renderWithProviders(<RetentionRules project={project('owner')} />);
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText(/What requires keeping the records/), 'Anti-money-laundering file');
  await user.type(screen.getByLabelText(/Legal reference/), 'COBAC R-2019/01');
  await user.type(screen.getByLabelText(/Last day/), '2020-01-01');
  await user.click(screen.getByRole('button', { name: 'Add the rule' }));
  expect(await screen.findByText('Choose today or a later day.')).toBeInTheDocument();
  expect(createMock).not.toHaveBeenCalled();
  await user.clear(screen.getByLabelText(/Last day/));
  await user.type(screen.getByLabelText(/Last day/), localToday());
  await user.click(screen.getByRole('button', { name: 'Add the rule' }));
  expect(createMock).toHaveBeenCalledWith('p-1', {
    description: 'Anti-money-laundering file',
    legalReference: 'COBAC R-2019/01',
    until: localToday(),
  });
});

it('lets owners end a running rule after confirming', async () => {
  endMock.mockResolvedValue({ ok: true, data: { ...rule, running: false, endedAt: '2026-10-28T09:00:00Z' } });
  renderWithProviders(<RetentionRules project={project('owner')} />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'End the rule' }));
  expect(screen.getByText('End this retention rule?')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'End the rule' }));
  expect(endMock).toHaveBeenCalledWith('p-1', 'rule-1');
});
