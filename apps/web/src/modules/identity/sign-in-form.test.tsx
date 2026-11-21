import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router, search } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { completeSignIn, confirmSignInEnrolment, getSession, signIn, startSignInEnrolment } from './identity.service';
import { SignInForm } from './sign-in-form';

vi.mock('./identity.service', () => ({
  getSession: vi.fn(),
  signIn: vi.fn(),
  completeSignIn: vi.fn(),
  startSignInEnrolment: vi.fn(),
  confirmSignInEnrolment: vi.fn(),
}));
const getSessionMock = vi.mocked(getSession);
const signInMock = vi.mocked(signIn);
const completeMock = vi.mocked(completeSignIn);
const startEnrolmentMock = vi.mocked(startSignInEnrolment);
const confirmEnrolmentMock = vi.mocked(confirmSignInEnrolment);
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

it('says when to try again after too many attempts', async () => {
  signInMock.mockResolvedValue({
    ok: false,
    error: { code: 'RATE_LIMITED', requestId: 'r-3', details: { retryAfterSeconds: 42 } },
  });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'password');
  expect(await screen.findByText('Too many attempts from your network. Try again in 42 seconds.')).toBeInTheDocument();
  expect(screen.queryByText(/r-3/)).not.toBeInTheDocument();
});

it('shows the reference for a failure on the server side', async () => {
  signInMock.mockResolvedValue({ ok: false, error: { code: 'UNEXPECTED', requestId: 'r-2' } });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'password');
  expect(await screen.findByText('Reference: r-2')).toBeInTheDocument();
});

it('returns to the page that asked for sign-in', async () => {
  signInMock.mockResolvedValue({ ok: true, data: { step: 'signedIn', user: ADA } });
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

// --- second step (DNK-59) ----------------------------------------------------------

it('asks for the code after the password and signs in with it', async () => {
  signInMock.mockResolvedValue({ ok: true, data: { step: 'code' } });
  completeMock.mockResolvedValue({ ok: true, data: ADA });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'correct horse battery staple');
  expect(await screen.findByText('Enter your code')).toBeInTheDocument();
  expect(router.replace).not.toHaveBeenCalled();

  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Code'), '123456');
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  expect(completeMock).toHaveBeenCalledWith({ code: '123456' });
  await waitFor(() => expect(router.replace).toHaveBeenCalledWith('/projects?id=7'));
});

it('keeps the code step after a wrong code and starts over when it expired', async () => {
  signInMock.mockResolvedValue({ ok: true, data: { step: 'code' } });
  completeMock.mockResolvedValueOnce({ ok: false, error: { code: 'INVALID_CREDENTIALS', requestId: 'r-4' } });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'correct horse battery staple');
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Code'), '000000');
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  expect(
    await screen.findByText('This code is not valid. Use the current code from your authenticator app.'),
  ).toBeInTheDocument();
  expect(screen.getByLabelText('Code')).toHaveValue('');

  completeMock.mockResolvedValueOnce({ ok: false, error: { code: 'SIGN_IN_EXPIRED', requestId: 'r-5' } });
  await user.type(screen.getByLabelText('Code'), '111111');
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  expect(await screen.findByRole('button', { name: 'Sign in' })).toBeInTheDocument();
  expect(screen.getByText('This sign-in has expired. Enter your email and password again.')).toBeInTheDocument();
});

it('enrols when policy requires it, then shows the recovery codes before Studio opens', async () => {
  signInMock.mockResolvedValue({ ok: true, data: { step: 'enrol' } });
  startEnrolmentMock.mockResolvedValue({
    ok: true,
    data: { key: 'JBSWY3DPEHPK3PXP', otpauthUrl: 'otpauth://totp/Donka:ada?secret=JBSWY3DPEHPK3PXP&issuer=Donka' },
  });
  const codes = Array.from({ length: 10 }, (_, i) => `ABCD-EFGH-JK${String(i).padStart(2, '0')}`);
  confirmEnrolmentMock.mockResolvedValue({ ok: true, data: { user: ADA, recoveryCodes: codes } });
  renderWithProviders(<SignInForm />);
  await submit('ada@bank.example', 'correct horse battery staple');

  expect(await screen.findByRole('img', { name: 'QR code to scan with your authenticator app' })).toBeInTheDocument();
  expect(screen.getByLabelText('Key, if you cannot scan')).toHaveValue('JBSWY3DPEHPK3PXP');
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Code from your authenticator app'), '123456');
  await user.click(screen.getByRole('button', { name: 'Turn on' }));

  expect(await screen.findByText('ABCD-EFGH-JK00')).toBeInTheDocument();
  const proceed = screen.getByRole('button', { name: 'Continue to Studio' });
  expect(proceed).toBeDisabled();
  expect(router.replace).not.toHaveBeenCalled();
  await user.click(screen.getByLabelText('I have saved my recovery codes'));
  await user.click(proceed);
  await waitFor(() => expect(router.replace).toHaveBeenCalledWith('/projects?id=7'));
});
