import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { Spinner } from '@/components/ui/spinner';

import { useStoryMessages } from './fixtures';

function ActionsPreview({ kind }: { kind: 'variants' | 'loading' | 'disabled' | 'choices' | 'toasts' }) {
  const m = useStoryMessages();
  const [choice, setChoice] = useState('light');
  if (kind === 'loading')
    return (
      <Button disabled>
        <Spinner />
        {m.common.saving}
      </Button>
    );
  if (kind === 'disabled') return <Button disabled>{m.common.save}</Button>;
  if (kind === 'choices')
    return (
      <RadioGroup aria-label={m.common.theme} value={choice} onValueChange={setChoice}>
        <div className="flex items-center gap-2">
          <RadioGroupItem id="choice-light" value="light" />
          <Label htmlFor="choice-light">{m.common.themeLight}</Label>
        </div>
        <div className="flex items-center gap-2">
          <RadioGroupItem id="choice-dark" value="dark" />
          <Label htmlFor="choice-dark">{m.common.themeDark}</Label>
        </div>
        <div className="flex items-center gap-2">
          <RadioGroupItem id="choice-system" value="system" disabled />
          <Label htmlFor="choice-system">{m.common.themeSystem}</Label>
        </div>
      </RadioGroup>
    );
  if (kind === 'toasts')
    return (
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => toast.success(m.project.saved)}>{m.common.save}</Button>
        <Button variant="destructive" onClick={() => toast.error(m.errors.NETWORK)}>
          {m.common.retry}
        </Button>
      </div>
    );
  return (
    <div className="flex flex-wrap gap-3">
      {(['default', 'secondary', 'outline', 'ghost', 'destructive', 'link'] as const).map((variant) => (
        <Button key={variant} variant={variant}>
          {m.common.save}
        </Button>
      ))}
    </div>
  );
}

const meta = { title: 'UI/Actions', component: ActionsPreview, args: { kind: 'variants' } } satisfies Meta<
  typeof ActionsPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Variants: Story = {};
export const Loading: Story = { args: { kind: 'loading' } };
export const Disabled: Story = { args: { kind: 'disabled' } };
export const RadioChoices: Story = { args: { kind: 'choices' } };
export const Toasts: Story = { args: { kind: 'toasts' } };
