import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { CircleCheck } from 'lucide-react';
import { useEffect, useState } from 'react';
import { z } from 'zod';

import { useAppForm } from '@/components/shared/form';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { FieldGroup } from '@/components/ui/field';

import { useStoryMessages, VIEW_STATES, type ViewState } from './fixtures';

const schema = z.object({
  name: z.string().min(1, 'required'),
  role: z.string().min(1, 'required'),
  notes: z.string(),
  required: z.boolean(),
});

function FormPreview({ state }: { state: ViewState }) {
  const m = useStoryMessages();
  const [saved, setSaved] = useState(false);
  const empty = state === 'empty' || state === 'error';
  const form = useAppForm({
    defaultValues: {
      name: empty ? '' : m.tokens.namePlaceholder,
      role: empty ? '' : 'editor',
      notes: '',
      required: true,
    },
    validators: { onChange: schema, onSubmit: schema },
    onSubmit: async () => {
      if (state === 'loading') await new Promise<void>(() => {});
      else setSaved(true);
    },
  });
  // Drive the same validation/submission states as a real form, without a service call.
  useEffect(() => {
    if (state !== 'empty') void form.handleSubmit();
  }, [form, state]);
  return (
    <div className="max-w-lg">
      <form.AppForm>
        <form.Form className="grid gap-4">
          {saved ? (
            <Alert>
              <CircleCheck aria-hidden />
              <AlertDescription>{m.project.saved}</AlertDescription>
            </Alert>
          ) : null}
          <FieldGroup>
            <form.AppField name="name">
              {(field) => (
                <field.TextField
                  label={m.projects.name}
                  placeholder={m.tokens.namePlaceholder}
                  disabled={state === 'loading'}
                  required
                />
              )}
            </form.AppField>
            <form.AppField name="role">
              {(field) => (
                <field.SelectField
                  label={m.members.role}
                  placeholder={m.members.role}
                  disabled={state === 'loading'}
                  options={[
                    { value: 'owner', label: m.roles.owner },
                    { value: 'editor', label: m.roles.editor },
                    { value: 'viewer', label: m.roles.viewer },
                  ]}
                />
              )}
            </form.AppField>
            <form.AppField name="notes">
              {(field) => (
                <field.TextField label={m.projects.description} multiline rows={3} disabled={state === 'loading'} />
              )}
            </form.AppField>
            <form.AppField name="required">
              {(field) => <field.CheckboxField label={m.inputFields.required} disabled={state === 'loading'} />}
            </form.AppField>
          </FieldGroup>
          <form.SubmitButton pendingLabel={m.common.saving}>{m.common.save}</form.SubmitButton>
        </form.Form>
      </form.AppForm>
    </div>
  );
}

const meta = {
  title: 'Shared/Form',
  component: FormPreview,
  args: { state: 'success' },
  argTypes: { state: { control: 'select', options: VIEW_STATES } },
  render: (args) => <FormPreview key={args.state} {...args} />,
} satisfies Meta<typeof FormPreview>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Loading: Story = { args: { state: 'loading' } };
export const EmptyState: Story = { args: { state: 'empty' } };
export const ErrorState: Story = { args: { state: 'error' } };
export const Success: Story = {};
