import { screen, within } from '@testing-library/react';

import { renderWithProviders } from '@/test/render';

import { getTestResults } from './decision.service';
import type { TestResult } from './schema';
import { TestBadge, TestResultsDialog } from './test-results';

vi.mock('./decision.service', () => ({ getTestResults: vi.fn() }));
const resultsMock = vi.mocked(getTestResults);

const base = {
  decisionKey: 'bureau/normalize',
  input: { input: 3 },
  expected: { output: 5 },
  match: 'partial' as const,
  mismatches: [],
};
const results: TestResult[] = [
  { ...base, scenarioId: 's-1', name: 'Above ten', status: 'passed', actual: { output: 10 } },
  {
    ...base,
    scenarioId: 's-2',
    name: 'Small',
    status: 'failed',
    actual: { output: 0 },
    mismatches: [
      { path: 'output', expected: 5, actual: 0 },
      { path: 'reason', expected: 'low' },
    ],
  },
  {
    ...base,
    scenarioId: 's-3',
    name: 'Score',
    decisionKey: 'person-score',
    status: 'error',
    missingDecision: 'person-score',
  },
];

beforeEach(() => vi.clearAllMocks());

it('sums up a version: all passed, or how many fail', () => {
  const view = renderWithProviders(<TestBadge summary={{ passed: 3, failed: 0, errors: 0 }} />);
  expect(screen.getByText('3 passed')).toBeInTheDocument();
  view.unmount();
  renderWithProviders(<TestBadge summary={{ passed: 1, failed: 1, errors: 1 }} />);
  expect(screen.getByText('2 of 3 failing')).toBeInTheDocument();
  expect(screen.getByLabelText('1 passed · 1 failed · 1 could not run')).toBeInTheDocument();
});

it('shows nothing when no scenario ran', () => {
  const { container } = renderWithProviders(<TestBadge summary={{ passed: 0, failed: 0, errors: 0 }} />);
  expect(container).toBeEmptyDOMElement();
});

it('shows failures first, with expected and actual per field', async () => {
  resultsMock.mockResolvedValue({ ok: true, data: { summary: { passed: 1, failed: 1, errors: 1 }, items: results } });
  renderWithProviders(<TestResultsDialog projectId="p-1" decisionId="d-1" number={2} onClose={vi.fn()} />);
  const dialog = await screen.findByRole('dialog', { name: 'Tests on version 2' });
  const items = await within(dialog).findAllByRole('listitem');
  expect(items.map((item) => within(item).getByText(/Above ten|Small|Score/).textContent)).toEqual([
    'Small',
    'Score',
    'Above ten',
  ]);
  const rows = within(items[0] as HTMLElement).getAllByRole('row');
  expect(rows[1]).toHaveTextContent('output50');
  expect(rows[2]).toHaveTextContent('reason"low"missing');
  expect(
    within(items[1] as HTMLElement).getByText('Could not run: person-score has no saved version yet.'),
  ).toBeVisible();
  expect(resultsMock).toHaveBeenCalledWith('p-1', 'd-1', 2);
});
