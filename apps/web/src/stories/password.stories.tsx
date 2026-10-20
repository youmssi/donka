import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { useState } from 'react';

import { useAppForm } from '@/components/shared/form';
import { Alert, AlertDescription } from '@/components/ui/alert';

import { useStoryMessages } from './fixtures';

function PasswordPreview({ disabled }: { disabled: boolean }) {
  const m = useStoryMessages();
  const [submitted, setSubmitted] = useState(false);
  const form = useAppForm({ defaultValues: { password: '', confirm: '' }, onSubmit: () => setSubmitted(true) });
  return (
    <div className="max-w-sm">
      <form.AppForm>
        <form.Form className="grid gap-4">
          {submitted ? (
            <Alert>
              <AlertDescription>{m.project.saved}</AlertDescription>
            </Alert>
          ) : null}
          <form.AppField name="password">
            {(field) => (
              <field.TextField
                label={m.signIn.password}
                type="password"
                autoComplete="current-password"
                disabled={disabled}
              />
            )}
          </form.AppField>
          <form.AppField name="confirm">
            {(field) => (
              <field.TextField
                label={m.setupPassword.confirm}
                type="password"
                autoComplete="new-password"
                disabled={disabled}
              />
            )}
          </form.AppField>
          <form.SubmitButton pendingLabel={m.common.saving} disabled={disabled}>
            {m.common.save}
          </form.SubmitButton>
        </form.Form>
      </form.AppForm>
    </div>
  );
}

const meta = { title: 'Shared/Password', component: PasswordPreview, args: { disabled: false } } satisfies Meta<
  typeof PasswordPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Default: Story = {};
export const Disabled: Story = { args: { disabled: true } };
