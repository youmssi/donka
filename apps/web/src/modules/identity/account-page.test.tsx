import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { AccountPage } from './account-page';
import {
  confirmEnrolment,
  disableSecondStep,
  getSecondStep,
  regenerateRecoveryCodes,
  startEnrolment,
} from './identity.service';

vi.mock('./identity.service', () => ({
  getSecondStep: vi.fn(),
  startEnrolment: vi.fn(),
  confirmEnrolment: vi.fn(),
  regenerateRecoveryCodes: vi.fn(),
  disableSecondStep: vi.fn(),
}));
vi.mock('./signed-in-frame', () => ({
  useCurrentUser: () => ({ id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' }),
}));
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

const statusMock = vi.mocked(getSecondStep);
const CODES = Array.from({ length: 10 }, (_, i) => `ABCD-EFGH-JK${String(i).padStart(2, '0')}`);

beforeEach(() => {
  vi.clearAllMocks();
});

it('turns two-step sign-in on and shows the recovery codes once', async () => {
  statusMock.mockResolvedValueOnce({ ok: true, data: { enabled: false, required: false, recoveryCodesLeft: 0 } });
  // Any later read of the status says it is on, as the server would.
  statusMock.mockResolvedValue({ ok: true, data: { enabled: true, required: false, recoveryCodesLeft: 10 } });
  vi.mocked(startEnrolment).mockResolvedValue({
    ok: true,
    data: { key: 'JBSWY3DPEHPK3PXP', otpauthUrl: 'otpauth://totp/Donka:ada?secret=JBSWY3DPEHPK3PXP' },
  });
  vi.mocked(confirmEnrolment).mockResolvedValue({ ok: true, data: CODES });
  renderWithProviders(<AccountPage />);
  expect(await screen.findByText('Signing in asks only for your password.')).toBeInTheDocument();

  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Turn on' }));
  await user.type(await screen.findByLabelText('Code from your authenticator app'), '123456');
  await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Turn on' }));
  await waitFor(() => expect(confirmEnrolment).toHaveBeenCalledWith({ code: '123456' }));
  expect(await screen.findByText('ABCD-EFGH-JK09')).toBeInTheDocument();
  const done = screen.getByRole('button', { name: 'Done' });
  expect(done).toBeDisabled();
  // The codes stay on screen until the person says they saved them.
  await user.click(screen.getByLabelText('I have saved my recovery codes'));
  expect(screen.getByText('ABCD-EFGH-JK09')).toBeInTheDocument();
  await user.click(done);
  expect(await screen.findByText('10 recovery codes left.')).toBeInTheDocument();
});

it('cannot turn off a second step the organisation requires', async () => {
  statusMock.mockResolvedValue({ ok: true, data: { enabled: true, required: true, recoveryCodesLeft: 2 } });
  renderWithProviders(<AccountPage />);
  expect(await screen.findByText('2 recovery codes left.')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Turn off' })).toBeDisabled();
  expect(screen.getByText(/it cannot be turned off/)).toBeInTheDocument();
});

it('turns it off with a recovery code and replaces the codes with an app code', async () => {
  statusMock.mockResolvedValue({ ok: true, data: { enabled: true, required: false, recoveryCodesLeft: 10 } });
  vi.mocked(disableSecondStep).mockResolvedValue({ ok: false, error: { code: 'INVALID_CODE', requestId: 'r-1' } });
  vi.mocked(regenerateRecoveryCodes).mockResolvedValue({ ok: true, data: CODES });
  renderWithProviders(<AccountPage />);
  const user = userEvent.setup();

  await user.click(await screen.findByRole('button', { name: 'Turn off' }));
  await user.type(await screen.findByLabelText('Code'), 'abcd-efgh-jk00');
  await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Turn off' }));
  await waitFor(() => expect(disableSecondStep).toHaveBeenCalledWith({ code: 'abcd-efgh-jk00' }));
  expect(
    await screen.findByText('This code is not valid. Use the current code from your authenticator app.'),
  ).toBeInTheDocument();
  await user.keyboard('{Escape}');

  await user.click(screen.getByRole('button', { name: 'New recovery codes' }));
  await user.type(await screen.findByLabelText('Code from your authenticator app'), '654321');
  await user.click(screen.getByRole('button', { name: 'Replace the codes' }));
  await waitFor(() => expect(regenerateRecoveryCodes).toHaveBeenCalledWith({ code: '654321' }));
  expect(await screen.findByText('ABCD-EFGH-JK00')).toBeInTheDocument();
});
