import type { ComponentProps } from 'react';

import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';

import { useFormContext } from './context';

/** The form's submit button: disabled with a spinner while the form is sent. */
export function SubmitButton({
  pendingLabel,
  children,
  ...props
}: ComponentProps<typeof Button> & { pendingLabel: string }) {
  const form = useFormContext();
  return (
    <form.Subscribe selector={(state) => state.isSubmitting}>
      {(pending) => (
        <Button type="submit" disabled={pending} {...props}>
          {pending ? <Spinner /> : null}
          {pending ? pendingLabel : children}
        </Button>
      )}
    </form.Subscribe>
  );
}
