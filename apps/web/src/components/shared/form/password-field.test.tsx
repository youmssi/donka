import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderWithProviders } from '@/test/render';

import { useAppForm } from '.';

function PasswordForm({ onSubmit, disabled = false }: { onSubmit: (value: unknown) => void; disabled?: boolean }) {
  const form = useAppForm({
    defaultValues: { password: '', confirm: '' },
    onSubmit: ({ value }) => onSubmit(value),
  });
  return (
    <form.AppForm>
      <form.Form>
        <form.AppField name="password">
          {(field) => (
            <field.TextField label="Password" type="password" autoComplete="current-password" disabled={disabled} />
          )}
        </form.AppField>
        <form.AppField name="confirm">
          {(field) => <field.TextField label="Confirm" type="password" autoComplete="new-password" />}
        </form.AppField>
        <form.SubmitButton pendingLabel="Saving…">Save</form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}

it('reveals with the keyboard without submitting or changing the value sent by the form', async () => {
  const onSubmit = vi.fn();
  const user = userEvent.setup();
  renderWithProviders(<PasswordForm onSubmit={onSubmit} />);
  const password = screen.getByLabelText('Password', { selector: 'input' });
  expect(password).toHaveAttribute('type', 'password');
  await user.type(password, 'a long verification phrase');
  await user.tab();
  const show = screen.getByRole('button', { name: 'Show password: Password' });
  expect(show).toHaveFocus();
  expect(show).toHaveAttribute('aria-controls', password.id);
  await user.keyboard(' ');
  expect(password).toHaveAttribute('type', 'text');
  expect(password).toHaveValue('a long verification phrase');
  expect(password).toHaveAttribute('autocomplete', 'current-password');
  expect(screen.getByLabelText('Confirm', { selector: 'input' })).toHaveAttribute('type', 'password');
  await user.keyboard('{Enter}');
  expect(password).toHaveAttribute('type', 'password');
  expect(onSubmit).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Save' }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith({ password: 'a long verification phrase', confirm: '' }));
});

it('toggles confirmation independently and keeps input focus when the eye is clicked', async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  renderWithProviders(<PasswordForm onSubmit={onSubmit} />);
  const confirm = screen.getByLabelText('Confirm', { selector: 'input' });
  await user.type(confirm, 'confirmation phrase');
  await user.click(screen.getByRole('button', { name: 'Show password: Confirm' }));
  expect(confirm).toHaveFocus();
  expect(confirm).toHaveAttribute('type', 'text');
  expect(confirm).toHaveAttribute('autocomplete', 'new-password');
  expect(screen.getByLabelText('Password', { selector: 'input' })).toHaveAttribute('type', 'password');
  await user.click(screen.getByRole('button', { name: 'Hide password: Confirm' }));
  expect(confirm).toHaveAttribute('type', 'password');
  expect(confirm).toHaveValue('confirmation phrase');
  expect(onSubmit).not.toHaveBeenCalled();
});

it('keeps a disabled password and its visibility button disabled', async () => {
  renderWithProviders(<PasswordForm onSubmit={vi.fn()} disabled />);
  expect(screen.getByLabelText('Password', { selector: 'input' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Show password: Password' })).toBeDisabled();
});
