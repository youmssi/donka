'use client';

import { useTranslations } from 'next-intl';
import { useId, type ReactNode } from 'react';

import { Field, FieldDescription, FieldError, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { cn } from '@/components/shared/utils';

import { useFieldContext } from './context';
import { shownError } from './text-field';

export interface SelectOption {
  value: string;
  label: ReactNode;
  className?: string;
}

interface SelectFieldProps {
  label: string;
  options: SelectOption[];
  hint?: ReactNode;
  placeholder?: string;
  disabled?: boolean;
  /** Classes for the `Field` around label, control and hint. */
  className?: string;
  triggerClassName?: string;
  /**
   * The option standing for an empty value: a select item cannot have `''` as its value, so this
   * one is stored as `''`.
   */
  emptyValue?: string;
  /** Called after the value changes, with the option picked. */
  onPick?: (value: string) => void;
}

/**
 * A choice bound to the surrounding `form.AppField`, laid out like {@link TextField}. A
 * validation message takes the hint's place once the form is submitted; opening the list moves
 * focus away from the trigger, so leaving it does not count as leaving the field.
 */
export function SelectField({
  label,
  options,
  hint,
  placeholder,
  disabled,
  className,
  triggerClassName,
  emptyValue,
  onPick,
}: SelectFieldProps) {
  const field = useFieldContext<string>();
  const t = useTranslations('validation');
  const id = useId();
  const key = shownError(field.state.meta, field.form.state.submissionAttempts);
  const value = emptyValue !== undefined && field.state.value === '' ? emptyValue : field.state.value;

  return (
    <Field className={cn('gap-2', className)} data-invalid={key ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select
        value={value}
        disabled={disabled}
        onValueChange={(picked) => {
          field.handleChange(picked === emptyValue ? '' : picked);
          onPick?.(picked);
        }}
      >
        <SelectTrigger
          id={id}
          className={cn('w-full', triggerClassName)}
          aria-invalid={key ? true : undefined}
          aria-describedby={key || hint ? `${id}-hint` : undefined}
        >
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value} className={option.className}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {key ? (
        <FieldError id={`${id}-hint`}>{t(key)}</FieldError>
      ) : hint ? (
        <FieldDescription id={`${id}-hint`}>{hint}</FieldDescription>
      ) : null}
    </Field>
  );
}
