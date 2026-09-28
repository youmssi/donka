import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { ForgotPasswordForm } from './forgot-password-form';
import { requestPasswordReset } from './identity.service';

vi.mock('./identity.service', () => ({ requestPasswordReset: vi.fn() }));
const resetMock = vi.mocked(requestPasswordReset);

beforeEach(() => resetMock.mockReset());

async function submit(email: string) {
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Email'), email);
  await user.click(screen.getByRole('button', { name: 'Send the link' }));
}

it('confirms without saying whether the address has an account', async () => {
  resetMock.mockResolvedValue({ ok: true, data: null });
  renderWithProviders(<ForgotPasswordForm />);
  await submit(' grace@bank.example ');
  expect(await screen.findByText(/If grace@bank.example has an account, a link is on its way/)).toBeInTheDocument();
  expect(resetMock).toHaveBeenCalledWith({ email: 'grace@bank.example' });
});

it('refuses something that is not an email address', async () => {
  renderWithProviders(<ForgotPasswordForm />);
  await submit('grace');
  expect(await screen.findByText('Enter a valid email address.')).toBeInTheDocument();
  expect(resetMock).not.toHaveBeenCalled();
});

it('says when Studio cannot be reached', async () => {
  resetMock.mockResolvedValue({ ok: false, error: { code: 'NETWORK' } });
  renderWithProviders(<ForgotPasswordForm />);
  await submit('grace@bank.example');
  expect(await screen.findByText(/Studio could not be reached/)).toBeInTheDocument();
});
