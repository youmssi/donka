import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { getProjectByKey } from '@/modules/project/project.service';
import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { createScenario, listDecisions, listScenarios } from './decision.service';
import { ScenariosPage } from './scenarios-page';
import type { Scenario } from './schema';

vi.mock('./decision.service', () => ({
  SCENARIOS_PAGE_SIZE: 100,
  VERSIONS_PAGE_SIZE: 20,
  listDecisions: vi.fn(),
  listScenarios: vi.fn(),
  createScenario: vi.fn(),
  updateScenario: vi.fn(),
  deleteScenario: vi.fn(),
}));
const decisionsMock = vi.mocked(listDecisions);
const listMock = vi.mocked(listScenarios);
const createMock = vi.mocked(createScenario);
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
const big: Scenario = {
  id: 's-1',
  decisionId: 'd-1',
  decisionKey: 'bureau/normalize',
  name: 'Income above ten',
  input: { input: 12 },
  expected: { output: 10 },
  match: 'partial',
  createdAt: '2026-09-29T09:00:00Z',
  createdBy: ada,
  updatedAt: '2026-09-29T09:00:00Z',
  updatedBy: ada,
};

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ p: 'credit-pme' });
  decisionsMock.mockResolvedValue({
    ok: true,
    data: [
      {
        id: 'd-1',
        key: 'bureau/normalize',
        revision: 1,
        updatedAt: '2026-09-29T09:00:00Z',
        updatedBy: ada,
        latestVersion: 1,
        changedSinceVersion: false,
      },
    ],
  });
  listMock.mockResolvedValue({ ok: true, data: { items: [big], total: 1 } });
});

it('lists the scenarios with the decision each one tests', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  renderWithProviders(<ScenariosPage />);
  const table = await screen.findByRole('table', { name: 'Test scenarios' });
  expect(await within(table).findByText('Income above ten')).toBeInTheDocument();
  expect(within(table).getByRole('link', { name: 'bureau/normalize' })).toHaveAttribute(
    'href',
    '/projects/decision?p=credit-pme&d=bureau%2Fnormalize',
  );
  expect(within(table).getByText('Partial')).toBeInTheDocument();
  expect(await screen.findByRole('button', { name: 'New scenario' })).toBeInTheDocument();
});

it('writes a scenario with JSON input and expected output', async () => {
  const user = userEvent.setup();
  projectMock.mockResolvedValue({ ok: true, data: project('editor') });
  createMock.mockResolvedValue({ ok: true, data: { ...big, id: 's-2', name: 'Small income' } });
  renderWithProviders(<ScenariosPage />);
  await user.click(await screen.findByRole('button', { name: 'New scenario' }));
  const dialog = await screen.findByRole('dialog', { name: 'New test scenario' });

  await user.type(within(dialog).getByLabelText('Name'), 'Small income');
  const input = within(dialog).getByLabelText('Input');
  await user.clear(input);
  await user.type(input, '[[1');
  await user.click(within(dialog).getByRole('button', { name: 'New scenario' }));
  expect(await within(dialog).findAllByText(/Enter a JSON object/)).not.toHaveLength(0);
  expect(createMock).not.toHaveBeenCalled();

  await user.clear(input);
  await user.type(input, '{{"input": 3}');
  const expected = within(dialog).getByLabelText('Expected output');
  await user.clear(expected);
  await user.type(expected, '{{"output": 0}');
  await user.click(within(dialog).getByRole('button', { name: 'New scenario' }));
  await waitFor(() =>
    expect(createMock).toHaveBeenCalledWith('p-1', 'd-1', {
      name: 'Small income',
      input: { input: 3 },
      expected: { output: 0 },
      match: 'partial',
    }),
  );
});

it('lets a viewer read scenarios without changing them', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('viewer') });
  renderWithProviders(<ScenariosPage />);
  await screen.findByText('Income above ten');
  expect(screen.queryByRole('button', { name: 'New scenario' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Actions for/ })).not.toBeInTheDocument();
});

it('says how to start when there is no scenario', async () => {
  projectMock.mockResolvedValue({ ok: true, data: project('owner') });
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<ScenariosPage />);
  expect(await screen.findByText('No test scenarios yet')).toBeInTheDocument();
});
