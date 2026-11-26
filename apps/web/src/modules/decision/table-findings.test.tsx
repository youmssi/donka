import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { toast } from 'sonner';

import { renderWithProviders } from '@/test/render';

import { acknowledgeFinding, listFindings } from './decision.service';
import type { TableFinding } from './schema';
import { TableChecksSheet } from './table-findings';

vi.mock('./decision.service', async (original) => ({
  ...(await original<typeof import('./decision.service')>()),
  listFindings: vi.fn(),
  acknowledgeFinding: vi.fn(),
}));
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
const listMock = vi.mocked(listFindings);

const project = (role: 'owner' | 'editor') => ({
  id: 'p-1',
  key: 'credit-pme',
  name: 'Crédit PME',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});

const base = { tableId: 'dt', tableName: 'Rate table', coveredBy: [], also: [], sameResult: false, cases: [] };
const gap: TableFinding = {
  ...base,
  fingerprint: 'gap',
  kind: 'gap',
  caseCount: 3,
  cases: [
    {
      conditions: [
        { column: { id: 'age', name: 'Age' }, condition: '[18..60]' },
        { column: { id: 'vip', name: 'VIP' }, condition: '!= true' },
      ],
      example: { applicant: { age: 18, vip: false } },
    },
    { conditions: [{ column: { id: 'age', name: 'Age' }, condition: '> 60' }] },
  ],
};
const overlap: TableFinding = {
  ...base,
  fingerprint: 'overlap',
  kind: 'overlap',
  row: 2,
  rowId: 'r2',
  coveredBy: [1],
  also: [4, 5],
  sameResult: true,
  example: { applicant: { age: 9 } },
};
const neverMatches: TableFinding = {
  ...base,
  fingerprint: 'never',
  kind: 'neverMatches',
  row: 4,
  rowId: 'r4',
  column: { id: 'age', name: 'Age' },
};
const incomplete: TableFinding = { ...base, fingerprint: 'incomplete', kind: 'incomplete', tableName: '' };
const acknowledgement = {
  reason: 'Rates under 18 are set by hand',
  acknowledgedBy: { id: 'u-1', email: 'ada@bank.example' },
  acknowledgedAt: '2026-10-28T09:00:00Z',
};

function Sheet({ role }: { role: 'owner' | 'editor' }) {
  const [open, setOpen] = useState(false);
  return (
    <TableChecksSheet project={project(role)} decisionId="d-1" savedAt={null} open={open} onOpenChange={setOpen} />
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

it('says in words what each kind of finding means, with the rows, columns and an example', async () => {
  listMock.mockResolvedValue({ ok: true, data: [gap, overlap, neverMatches, incomplete] });
  renderWithProviders(<Sheet role="editor" />);
  const user = userEvent.setup();
  // The button counts what nobody has acknowledged yet.
  await user.click(await screen.findByRole('button', { name: /Table checks.*4 findings to look at/ }));
  const sheet = await screen.findByRole('dialog', { name: 'Table checks' });

  expect(within(sheet).getByText('Rate table: no row matches 3 kinds of input')).toBeInTheDocument();
  expect(within(sheet).getByText('Age [18..60] · VIP != true')).toBeInTheDocument();
  expect(within(sheet).getByText('{"applicant":{"age":18,"vip":false}}')).toBeInTheDocument();
  expect(within(sheet).getByText('Other cases not shown: 1')).toBeInTheDocument();

  expect(
    within(sheet).getByText('Rate table, row 2: never used, because row 1 already matches every input it matches'),
  ).toBeInTheDocument();
  expect(within(sheet).getByText(/same result, so it can be removed/)).toBeInTheDocument();
  expect(within(sheet).getByText('Rows 4, 5 below are never used for the same reason.')).toBeInTheDocument();

  expect(
    within(sheet).getByText('Rate table, row 4: no input can meet the Age condition, so the row is never used'),
  ).toBeInTheDocument();
  expect(within(sheet).getByText(/Unnamed table: too large to check completely/)).toBeInTheDocument();
  // Only owners acknowledge.
  expect(within(sheet).queryByRole('button', { name: 'Acknowledge' })).not.toBeInTheDocument();
});

it('lets an owner acknowledge a finding with a reason', async () => {
  listMock.mockResolvedValueOnce({ ok: true, data: [overlap] });
  listMock.mockResolvedValue({ ok: true, data: [{ ...overlap, acknowledgement }] });
  vi.mocked(acknowledgeFinding).mockResolvedValue({ ok: true, data: { ...overlap, acknowledgement } });
  renderWithProviders(<Sheet role="owner" />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: /Table checks.*1 finding to look at/ }));
  const sheet = await screen.findByRole('dialog', { name: 'Table checks' });
  await user.click(within(sheet).getByRole('button', { name: 'Acknowledge' }));

  const dialog = await screen.findByRole('dialog', { name: 'Acknowledge this finding' });
  await user.click(within(dialog).getByRole('button', { name: 'Acknowledge' }));
  expect(await within(dialog).findByText('This field is required.')).toBeInTheDocument();
  expect(acknowledgeFinding).not.toHaveBeenCalled();

  await user.type(within(dialog).getByLabelText(/Why it is intended/), '  Rates under 18 are set by hand ');
  await user.click(within(dialog).getByRole('button', { name: 'Acknowledge' }));
  await waitFor(() =>
    expect(acknowledgeFinding).toHaveBeenCalledWith('p-1', 'd-1', 'overlap', 'Rates under 18 are set by hand'),
  );
  expect(toast.success).toHaveBeenCalledWith('Finding acknowledged');
  // The list reads the findings again: the finding shows who acknowledged it and why.
  expect(await screen.findByText('ada@bank.example: “Rates under 18 are set by hand”')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Acknowledge' })).not.toBeInTheDocument();
});

it('says when the tables have nothing to report', async () => {
  listMock.mockResolvedValue({ ok: true, data: [] });
  renderWithProviders(<Sheet role="owner" />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'Table checks' }));
  expect(await screen.findByText('Nothing to report')).toBeInTheDocument();
});
