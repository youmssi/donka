import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { z } from 'zod';

import { renderWithProviders } from '@/test/render';

import { useAppForm } from '.';

const schema = z.object({
  name: z.string().min(1, 'required'),
  key: z.string().min(1, 'required'),
  role: z.string().min(1, 'required'),
});

function Example({ onSubmit }: { onSubmit: (value: z.infer<typeof schema>) => void }) {
  const form = useAppForm({
    defaultValues: { name: 'Credit SME', key: '', role: '' },
    validators: { onChange: schema, onSubmit: schema },
    onSubmit: ({ value }) => onSubmit(value),
  });
  return (
    <form.AppForm>
      <form.Form>
        <form.AppField name="name">{(field) => <field.TextField label="Name" />}</form.AppField>
        <form.AppField name="key">{(field) => <field.TextField label="Key" />}</form.AppField>
        <form.AppField name="role">
          {(field) => (
            <field.SelectField
              label="Role"
              hint="What they can do"
              options={[
                { value: 'editor', label: 'Editor' },
                { value: 'owner', label: 'Owner' },
              ]}
            />
          )}
        </form.AppField>
        <form.SubmitButton pendingLabel="Saving…">Save</form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}

it('moves focus to the first field in error when a submission is refused', async () => {
  const onSubmit = vi.fn();
  renderWithProviders(<Example onSubmit={onSubmit} />);
  await userEvent.setup().click(screen.getByRole('button', { name: 'Save' }));

  await waitFor(() => expect(screen.getByLabelText('Key')).toHaveFocus());
  expect(screen.getByLabelText('Key')).toHaveAttribute('aria-invalid', 'true');
  expect(screen.getByRole('combobox', { name: 'Role' })).toHaveAttribute('aria-invalid', 'true');
  expect(screen.getAllByText('This field is required.')).toHaveLength(2);
  expect(screen.queryByText('What they can do')).not.toBeInTheDocument();
  expect(onSubmit).not.toHaveBeenCalled();
});

it('submits once every field is valid, without moving focus', async () => {
  const onSubmit = vi.fn();
  const user = userEvent.setup();
  renderWithProviders(<Example onSubmit={onSubmit} />);
  await user.type(screen.getByLabelText('Key'), 'credit-sme');
  await user.click(screen.getByRole('combobox', { name: 'Role' }));
  await user.click(await screen.findByRole('option', { name: 'Owner' }));
  expect(screen.getByText('What they can do')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Save' }));

  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith({ name: 'Credit SME', key: 'credit-sme', role: 'owner' }));
  expect(screen.getByRole('button', { name: 'Save' })).toHaveFocus();
});
