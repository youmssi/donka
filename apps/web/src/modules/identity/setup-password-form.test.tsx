import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { setPassword } from './identity.service';
import { SetupPasswordForm } from './setup-password-form';

vi.mock('./identity.service', () => ({ setPassword: vi.fn() }));
const setPasswordMock = vi.mocked(setPassword);

const GOOD = 'correct horse battery staple';

beforeEach(() => {
  setPasswordMock.mockReset();
  search.params = new URLSearchParams({ token: 'tok-1' });
});

async function fill(password: string, confirm: string) {
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('New password'), password);
  await user.type(screen.getByLabelText('Confirm the password'), confirm);
  await user.click(screen.getByRole('button', { name: 'Save the password' }));
}

it('explains what to do when opened without a link', () => {
  search.params = new URLSearchParams();
  renderWithProviders(<SetupPasswordForm />);
  expect(screen.getByText(/This page needs the link from your email/)).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Ask for a new link' })).toHaveAttribute('href', '/forgot-password');
});

it('checks length and confirmation before calling the server', async () => {
  renderWithProviders(<SetupPasswordForm />);
  await fill('short', 'other');
  expect(await screen.findByText('Use 12 to 128 characters.')).toBeInTheDocument();
  expect(screen.getByText('The passwords do not match.')).toBeInTheDocument();
  expect(setPasswordMock).not.toHaveBeenCalled();
});

it('sets the password and offers to sign in', async () => {
  setPasswordMock.mockResolvedValue({ ok: true, data: null });
  renderWithProviders(<SetupPasswordForm />);
  await fill(GOOD, GOOD);
  expect(await screen.findByText('Your password is set')).toBeInTheDocument();
  expect(setPasswordMock).toHaveBeenCalledWith('tok-1', GOOD);
  expect(screen.getByRole('link', { name: 'Sign in' })).toHaveAttribute('href', '/sign-in');
});

it('offers a new link when this one no longer works, keeping what was typed', async () => {
  setPasswordMock.mockResolvedValue({ ok: false, error: { code: 'INVALID_SETUP_LINK', requestId: 'r-9' } });
  renderWithProviders(<SetupPasswordForm />);
  await fill(GOOD, GOOD);
  expect(await screen.findByText(/This link is invalid, already used or expired/)).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Ask for a new link' })).toBeInTheDocument();
  expect(screen.queryByText(/r-9/)).not.toBeInTheDocument();
  expect(screen.getByLabelText('New password')).toHaveValue(GOOD);
});
