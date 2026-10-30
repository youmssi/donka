import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { router } from '@/test/navigation-mock';
import { renderWithProviders } from '@/test/render';

import { FirstRunAssistant } from './first-run-assistant';
import { finishFirstRun, getInstallation } from './installation.service';

vi.mock('./installation.service', () => ({ getInstallation: vi.fn(), finishFirstRun: vi.fn() }));
const getMock = vi.mocked(getInstallation);
const finishMock = vi.mocked(finishFirstRun);

beforeEach(() => {
  vi.clearAllMocks();
  getMock.mockResolvedValue({ ok: true, data: { firstRun: true, organisation: null } });
});

/** The assistant with stand-ins for the second step and the packs, each a button calling `next`. */
function renderAssistant(isAdmin = true) {
  renderWithProviders(
    <FirstRunAssistant
      isAdmin={isAdmin}
      secondStep={(next, label) => (
        <button type="button" onClick={next}>
          {`second step: ${label}`}
        </button>
      )}
      packs={(next, label) => (
        <button type="button" onClick={next}>
          {`packs: ${label}`}
        </button>
      )}
    />,
  );
}

it('goes through the second step, the organisation and the packs, then finishes', async () => {
  finishMock.mockResolvedValue({ ok: true, data: { firstRun: false, organisation: 'Microfinance SA' } });
  const user = userEvent.setup();
  renderAssistant();
  expect(await screen.findByRole('heading', { name: 'Set up Donka' })).toBeInTheDocument();
  expect(screen.getByRole('listitem', { current: 'step' })).toHaveTextContent('Two-step sign-in');

  await user.click(screen.getByRole('button', { name: 'second step: Continue' }));
  expect(screen.getByRole('listitem', { current: 'step' })).toHaveTextContent('Organisation');
  const name = screen.getByLabelText(/^Organisation name/);
  await user.click(screen.getByRole('button', { name: 'Continue' }));
  expect(await screen.findByText('This field is required.')).toBeInTheDocument();
  await user.type(name, '  Microfinance SA ');
  await user.click(screen.getByRole('button', { name: 'Continue' }));

  expect(screen.getByRole('listitem', { current: 'step' })).toHaveTextContent('Packs');
  await user.click(screen.getByRole('button', { name: 'packs: Finish setting up' }));
  await waitFor(() => expect(finishMock).toHaveBeenCalledWith({ organisation: 'Microfinance SA' }));
  expect(router.replace).toHaveBeenCalledWith('/');
});

it('says why it cannot finish, and stays', async () => {
  finishMock.mockResolvedValue({
    ok: false,
    error: { code: 'SECOND_STEP_REQUIRED' },
  });
  const user = userEvent.setup();
  renderAssistant();
  await user.click(await screen.findByRole('button', { name: 'second step: Continue' }));
  await user.type(screen.getByLabelText(/^Organisation name/), 'Microfinance SA');
  await user.click(screen.getByRole('button', { name: 'Continue' }));
  await user.click(screen.getByRole('button', { name: 'packs: Finish setting up' }));
  expect(await screen.findByRole('alert')).toBeInTheDocument();
  expect(router.replace).not.toHaveBeenCalled();
});

it('shows only during the first run, and only to administrators', async () => {
  getMock.mockResolvedValue({ ok: true, data: { firstRun: false, organisation: 'Microfinance SA' } });
  renderAssistant();
  expect(await screen.findByText('Donka is set up')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /second step/ })).not.toBeInTheDocument();
});

it('leaves the setup to administrators', async () => {
  renderAssistant(false);
  expect(await screen.findByText('An administrator sets up Donka')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Go to Studio' })).toHaveAttribute('href', '/');
});
