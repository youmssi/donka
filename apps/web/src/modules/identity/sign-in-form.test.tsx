import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { getSession, signIn } from './identity.service';
import { SignInForm } from './sign-in-form';

vi.mock('./identity.service', () => ({ getSession: vi.fn(), signIn: vi.fn() }));
const getSessionMock = vi.mocked(getSession);
const signInMock = vi.mocked(signIn);
const ADA = { id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' as const };

beforeEach(() => {
  vi.clearAllMocks();
  search.params = new URLSearchParams({ next: '/projects?id=7' });
  getSessionMock.mockResolvedValue({ ok: true, data: null });
});

async function submit(email: string, password: string) {
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Email'), email);
  await user.type(screen.getByLabelText('Password'), password);
  await user.click(screen.getByRole('button', { name: 'Sign in' }));
}

it('shows a loading state while it checks for an existing session', () => {
  getSessionMock.mockReturnValue(new Promise(() => {}));
  renderWithProviders(<SignInForm />);
  expect(screen.getByRole('status')).toHaveTextContent('Loading…');
});

it('asks for both fields before calling the server', async () => {
  renderWithProviders(<SignInForm />);
  await userEvent.setup().click(await screen.findByRole('button', { name: 'Sign in' }));
  expect(await screen.findAllByText('This field is required.')).toHaveLength(2);
  expect(signInMock).not.toHaveBeenCalled();
});

it('shows one message for wrong credentials, without a reference', async () => {
  signInMock.mockResolvedValue({ ok: false, error: { code: 'INVALID_CREDENTIALS', requestId: 'r-1' } });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'wrong password');
  expect(await screen.findByText('Email or password is incorrect.')).toBeInTheDocument();
  expect(screen.queryByText(/r-1/)).not.toBeInTheDocument();
  expect(router.replace).not.toHaveBeenCalled();
});

it('shows the reference for a failure on the server side', async () => {
  signInMock.mockResolvedValue({ ok: false, error: { code: 'UNEXPECTED', requestId: 'r-2' } });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'password');
  expect(await screen.findByText('Reference: r-2')).toBeInTheDocument();
});

it('returns to the page that asked for sign-in', async () => {
  signInMock.mockResolvedValue({ ok: true, data: ADA });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'correct horse battery staple');
  await waitFor(() => expect(router.replace).toHaveBeenCalledWith('/projects?id=7'));
  expect(signInMock).toHaveBeenCalledWith({ email: 'ada@bank.example', password: 'correct horse battery staple' });
});

it('ignores a next parameter that points to another site', async () => {
  search.params = new URLSearchParams({ next: '//evil.example' });
  getSessionMock.mockResolvedValue({ ok: true, data: ADA });
  renderWithProviders(<SignInForm />);
  await waitFor(() => expect(router.replace).toHaveBeenCalledWith('/'));
});
