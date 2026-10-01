import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { listVersions, restoreVersion, saveVersion } from './decision.service';
import type { Decision } from './schema';
import type { useDraft } from './useDraft';
import { HistorySheet, SaveVersionDialog, VersionChip } from './versions';

vi.mock('./decision.service', () => ({
  VERSIONS_PAGE_SIZE: 20,
  listVersions: vi.fn(),
  getVersion: vi.fn(),
  saveVersion: vi.fn(),
  restoreVersion: vi.fn(),
}));
const listMock = vi.mocked(listVersions);
const saveMock = vi.mocked(saveVersion);
const restoreMock = vi.mocked(restoreVersion);

const decision: Decision = {
  id: 'd-1',
  key: 'person-score',
  revision: 4,
  updatedAt: '2026-09-29T09:00:00Z',
  updatedBy: { id: 'u-1', email: 'ada@bank.example' },
  latestVersion: 2,
  changedSinceVersion: true,
  content: { nodes: [], edges: [] },
};
const ada = { id: 'u-1', email: 'ada@bank.example' };
const [v2, v1] = [
  { number: 2, message: 'Raise the threshold', createdAt: '2026-09-29T08:00:00Z', createdBy: ada, restoredFrom: null },
  { number: 1, message: 'First table', createdAt: '2026-09-28T08:00:00Z', createdBy: ada, restoredFrom: null },
];
const history = [v2, v1];

type Draft = ReturnType<typeof useDraft>;
function fakeDraft(version: Draft['version'] = { latest: 2, changed: true }): Draft {
  return {
    graph: decision.content,
    status: { kind: 'saved', at: decision.updatedAt },
    version,
    change: vi.fn(),
    keepMine: vi.fn(),
    loadTheirs: vi.fn(),
    retry: vi.fn(),
    settle: vi.fn().mockResolvedValue(true),
    replace: vi.fn(),
    versionSaved: vi.fn(),
    currentRevision: () => 4,
  };
}
const shared = {
  projectId: 'p-1',
  decision,
  callable: [],
  decisionNodeLabels: { displayName: '', shortDescription: '', choose: '' },
};

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockResolvedValue({ ok: true, data: { items: history, total: 2 } });
});

it('shows where the draft stands in the history', () => {
  const view = renderWithProviders(<VersionChip latest={null} changed />);
  expect(screen.getByText('No version yet')).toBeInTheDocument();
  view.unmount();
  renderWithProviders(<VersionChip latest={3} changed />);
  expect(screen.getByText('v3')).toBeInTheDocument();
  expect(screen.getByText(/changed since/)).toBeInTheDocument();
});

it('saves a version of what is on screen, with a required message', async () => {
  const user = userEvent.setup();
  saveMock.mockResolvedValue({ ok: true, data: { ...v2, number: 3, message: 'Add bureau score' } });
  const draft = fakeDraft();
  renderWithProviders(<SaveVersionDialog projectId="p-1" decision={decision} draft={draft} />);
  await user.click(screen.getByRole('button', { name: 'Save version' }));
  const dialog = await screen.findByRole('dialog', { name: 'Save version 3' });

  await user.click(within(dialog).getByRole('button', { name: 'Save version' }));
  expect(await within(dialog).findByText(/required/i)).toBeInTheDocument();
  expect(saveMock).not.toHaveBeenCalled();

  await user.type(within(dialog).getByLabelText('What changed'), '  Add bureau score ');
  await user.click(within(dialog).getByRole('button', { name: 'Save version' }));
  await waitFor(() => expect(saveMock).toHaveBeenCalledWith('p-1', 'd-1', 'Add bureau score', 4));
  expect(draft.settle).toHaveBeenCalled();
  expect(draft.versionSaved).toHaveBeenCalledWith(3);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});

it('does not offer to save a version when nothing changed', () => {
  renderWithProviders(
    <SaveVersionDialog projectId="p-1" decision={decision} draft={fakeDraft({ latest: 2, changed: false })} />,
  );
  expect(screen.getByRole('button', { name: 'Save version' })).toBeDisabled();
});

it('lists versions newest first and restores one into the draft', async () => {
  const user = userEvent.setup();
  const restored = { ...decision, revision: 5, latestVersion: 3, changedSinceVersion: false };
  restoreMock.mockResolvedValue({
    ok: true,
    data: {
      decision: restored,
      version: { ...v1, number: 3, message: 'Restored version 1', restoredFrom: 1 },
    },
  });
  const draft = fakeDraft();
  renderWithProviders(<HistorySheet {...shared} draft={draft} editable />);
  await user.click(screen.getByRole('button', { name: 'History' }));
  const sheet = await screen.findByRole('dialog', { name: 'Version history' });
  const items = await within(sheet).findAllByRole('listitem');
  expect(items.map((item) => within(item).getByText(/^v\d$/).textContent)).toEqual(['v2', 'v1']);
  expect(within(sheet).getByText('Raise the threshold')).toBeInTheDocument();

  await user.click(within(sheet).getByRole('button', { name: 'Actions for version 1' }));
  await user.click(await screen.findByRole('menuitem', { name: 'Restore' }));
  const confirm = await screen.findByRole('alertdialog', { name: 'Restore version 1?' });
  await user.click(within(confirm).getByRole('button', { name: 'Restore' }));
  await waitFor(() => expect(restoreMock).toHaveBeenCalledWith('p-1', 'd-1', 1, 4));
  expect(draft.replace).toHaveBeenCalledWith(restored);
});

it('lets a viewer compare but not restore', async () => {
  const user = userEvent.setup();
  renderWithProviders(<HistorySheet {...shared} draft={fakeDraft()} editable={false} />);
  await user.click(screen.getByRole('button', { name: 'History' }));
  const sheet = await screen.findByRole('dialog', { name: 'Version history' });
  await user.click(await within(sheet).findByRole('button', { name: 'Actions for version 2' }));
  expect(await screen.findByRole('menuitem', { name: 'Compare with version 1' })).toBeInTheDocument();
  expect(screen.queryByRole('menuitem', { name: 'Restore' })).not.toBeInTheDocument();
});

it('says so when there is no version yet', async () => {
  const user = userEvent.setup();
  listMock.mockResolvedValue({ ok: true, data: { items: [], total: 0 } });
  renderWithProviders(<HistorySheet {...shared} draft={fakeDraft({ latest: null, changed: true })} editable />);
  await user.click(screen.getByRole('button', { name: 'History' }));
  expect(await screen.findByText('No versions yet')).toBeInTheDocument();
});
