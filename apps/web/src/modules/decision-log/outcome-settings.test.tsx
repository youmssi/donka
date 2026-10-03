import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { getSettings, saveSettings } from './decision-log.service';
import { OutcomeSettings } from './outcome-settings';

vi.mock('./decision-log.service', async (original) => ({
  ...(await original<typeof import('./decision-log.service')>()),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
}));
const getMock = vi.mocked(getSettings);
const saveMock = vi.mocked(saveSettings);

const project = (role: 'owner' | 'viewer') => ({
  id: 'p-1',
  key: 'retail',
  name: 'Retail',
  description: '',
  createdAt: '2026-09-28T09:00:00Z',
  archivedAt: null,
  role,
});

beforeEach(() => {
  vi.clearAllMocks();
  getMock.mockResolvedValue({
    ok: true,
    data: { outcomeField: 'decision', redactedFields: ['applicant.nationalId'], explainEnabled: true },
  });
});

it('lets owners list the fields never sent for an explanation, one per line', async () => {
  saveMock.mockResolvedValue({
    ok: true,
    data: { outcomeField: 'decision', redactedFields: ['applicant.nationalId', 'iban'], explainEnabled: true },
  });
  renderWithProviders(<OutcomeSettings project={project('owner')} />);
  const fields = await screen.findByRole('textbox', { name: /Fields never sent/ });
  expect(fields).toHaveValue('applicant.nationalId');
  expect(screen.getByText(/removed from what the decision read and answered/)).toBeInTheDocument();
  const user = userEvent.setup();
  await user.type(fields, '\n iban \napplicant.nationalId');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(saveMock).toHaveBeenCalledWith('p-1', {
    outcomeField: 'decision',
    redactedFields: ['applicant.nationalId', 'iban'],
  });
});

it('refuses a line that is not a field', async () => {
  renderWithProviders(<OutcomeSettings project={project('owner')} />);
  const fields = await screen.findByRole('textbox', { name: /Fields never sent/ });
  const user = userEvent.setup();
  await user.type(fields, '\nnational id');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(await screen.findByText(/Write one field per line/)).toBeInTheDocument();
  expect(saveMock).not.toHaveBeenCalled();
});

it('shows the list read-only to others, and says when explanations are off', async () => {
  getMock.mockResolvedValue({
    ok: true,
    data: { outcomeField: null, redactedFields: [], explainEnabled: false },
  });
  renderWithProviders(<OutcomeSettings project={project('viewer')} />);
  expect(await screen.findByRole('textbox', { name: /Fields never sent/ })).toBeDisabled();
  expect(screen.getByText(/Explanations are off in this installation/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Save' })).not.toBeInTheDocument();
});
