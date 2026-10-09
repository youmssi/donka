'use client';

import { useId } from 'react';

import { Checkbox } from '@/components/ui/checkbox';
import { Field, FieldContent, FieldDescription, FieldLabel } from '@/components/ui/field';

import { useFieldContext } from './context';

interface CheckboxFieldProps {
  label: string;
  hint?: string;
  disabled?: boolean;
  className?: string;
  labelClassName?: string;
}

/** A yes/no choice bound to the surrounding `form.AppField`: the box, its label, and a hint under it. */
export function CheckboxField({ label, hint, disabled, className, labelClassName }: CheckboxFieldProps) {
  const field = useFieldContext<boolean>();
  const id = useId();
  const labelled = (
    <FieldLabel htmlFor={id} className={labelClassName}>
      {label}
    </FieldLabel>
  );

  return (
    <Field orientation="horizontal" className={className}>
      <Checkbox
        id={id}
        checked={field.state.value}
        disabled={disabled}
        onCheckedChange={(checked) => field.handleChange(checked === true)}
        onBlur={field.handleBlur}
        aria-describedby={hint ? `${id}-hint` : undefined}
      />
      {hint ? (
        <FieldContent>
          {labelled}
          <FieldDescription id={`${id}-hint`}>{hint}</FieldDescription>
        </FieldContent>
      ) : (
        labelled
      )}
    </Field>
  );
}
