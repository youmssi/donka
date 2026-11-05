import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { RefusedConnectors } from './refused-connectors';

it('names each connector the Runtime will refuse and why, and can be dismissed', async () => {
  const user = userEvent.setup();
  const onDismiss = vi.fn();
  renderWithProviders(
    <RefusedConnectors
      number={4}
      connectors={[
        { node: 'Bureau', host: 'attacker.example', secret: null },
        { node: 'Core banking', host: 'cbs.bank', secret: 'CBS_CLIENT_SECRET' },
      ]}
      onDismiss={onDismiss}
    />,
  );
  expect(screen.getByText('Version 4: connectors the Runtime will refuse')).toBeInTheDocument();
  expect(screen.getByText('Bureau calls attacker.example, where no connector may call.')).toBeInTheDocument();
  expect(
    screen.getByText('Core banking sends the secret CBS_CLIENT_SECRET to cbs.bank, where that secret may not go.'),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Dismiss' }));
  expect(onDismiss).toHaveBeenCalled();
});
