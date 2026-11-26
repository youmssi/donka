import { screen } from '@testing-library/react';

import { renderWithProviders } from '@/test/render';

import { GetStarted } from './get-started';
import { getOnboarding } from './onboarding.service';
import type { Onboarding, Step } from './schema';

vi.mock('./onboarding.service', () => ({ getOnboarding: vi.fn() }));
const user = { id: 'u-1', email: 'ada@bank.example', isAdmin: true, locale: 'en' as const };
vi.mock('@/modules/identity', () => ({ useCurrentUser: () => user }));

const ORDER: Step[] = ['project', 'simulation', 'version', 'staging', 'token'];
function state(done: number, project: Onboarding['project'] = null): Onboarding {
  return {
    steps: ORDER.map((step, index) => ({ step, done: index < done })),
    project,
    complete: done === ORDER.length,
  };
}
const retail = { key: 'retail-credit', name: 'Retail credit' };

beforeEach(() => {
  user.isAdmin = true;
});

it('starts an administrator with a starter pack', async () => {
  vi.mocked(getOnboarding).mockResolvedValue({ ok: true, data: state(0) });
  renderWithProviders(<GetStarted />);
  expect(await screen.findByRole('heading', { name: 'Get started' })).toBeInTheDocument();
  expect(screen.getByText(/0 of 5 done/)).toBeInTheDocument();
  expect(screen.getAllByRole('listitem')).toHaveLength(5);
  expect(screen.getByRole('button', { name: 'From a pack' })).toBeInTheDocument();
  // Only the next step offers a way to do it.
  expect(screen.queryByRole('link')).not.toBeInTheDocument();
});

it('tells someone who cannot create projects whom to ask', async () => {
  user.isAdmin = false;
  vi.mocked(getOnboarding).mockResolvedValue({ ok: true, data: state(0) });
  renderWithProviders(<GetStarted />);
  expect(await screen.findByText(/Ask an administrator to start a project/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'From a pack' })).not.toBeInTheDocument();
});

it('marks what is done and links the next step to its place in the project', async () => {
  vi.mocked(getOnboarding).mockResolvedValue({ ok: true, data: state(3, retail) });
  renderWithProviders(<GetStarted />);
  expect(await screen.findByText(/3 of 5 done/)).toBeInTheDocument();
  expect(screen.getByText('Run a simulation')).toHaveTextContent('(done)');
  expect(screen.getByText('Release it to staging')).toHaveTextContent('(to do)');
  expect(screen.getByRole('link', { name: 'Open the releases of Retail credit' })).toHaveAttribute(
    'href',
    '/projects/releases?p=retail-credit',
  );
});

it('disappears once every step is done', async () => {
  vi.mocked(getOnboarding).mockResolvedValue({ ok: true, data: state(5, retail) });
  const { container } = renderWithProviders(<GetStarted />);
  await vi.waitFor(() => expect(getOnboarding).toHaveBeenCalled());
  expect(container).toBeEmptyDOMElement();
});
