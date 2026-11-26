import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { CreateProjectDialog } from './create-project-dialog';

vi.mock('./project.service', () => ({ createProject: vi.fn() }));

it('proposes the key from the whole name until the person edits it', async () => {
  const user = userEvent.setup();
  renderWithProviders(<CreateProjectDialog />);
  await user.click(screen.getByRole('button', { name: 'New project' }));
  const dialog = await screen.findByRole('dialog');
  const key = within(dialog).getByLabelText(/^Key/);
  await user.type(within(dialog).getByLabelText(/^Name/), 'Crédit PME');
  expect(key).toHaveValue('credit-pme');
  await user.clear(key);
  await user.type(key, 'sme');
  await user.type(within(dialog).getByLabelText(/^Name/), ' 2026');
  expect(key).toHaveValue('sme');
});
