'use client';

import type { AnyFieldApi } from '@tanstack/react-form';
import { useTranslations } from 'next-intl';
import { useId, type ComponentProps } from 'react';

import { Field, FieldDescription, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';

interface TextFieldProps extends Omit<ComponentProps<typeof Input>, 'name' | 'value' | 'onChange' | 'onBlur'> {
  field: AnyFieldApi;
  label: string;
  hint?: string;
  /** Values for placeholders in validation messages, e.g. `{ min: 12 }`. */
  messageValues?: Record<string, number | string>;
  /** Message sent back by the server for this field, already translated. */
  serverError?: string;
  /** A text area instead of a one-line input. */
  multiline?: boolean;
}

/** The message key of the first error: Zod issues carry a key under `validation`. */
export function firstMessage(errors: unknown[]): string | undefined {
  for (const error of errors) {
    if (typeof error === 'string') return error;
    if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') {
      return error.message;
    }
  }
  return undefined;
}

/**
 * A text input bound to a TanStack Form field, laid out with shadcn `Field`.
 * Errors appear after the person leaves the field or submits, never while they
 * are still typing.
 */
export function TextField({
  field,
  label,
  hint,
  messageValues,
  serverError,
  multiline,
  ...inputProps
}: TextFieldProps) {
  const t = useTranslations('validation');
  const id = useId();
  const meta = field.state.meta;
  const shown = meta.isBlurred || field.form.state.submissionAttempts > 0;
  const key = shown ? firstMessage(meta.errors) : undefined;
  const error = key ? t(key, messageValues) : serverError;
  const describedBy = [hint ? `${id}-hint` : null, `${id}-error`].filter(Boolean).join(' ');
  const control = {
    id,
    name: field.name,
    value: String(field.state.value ?? ''),
    onBlur: field.handleBlur,
    'aria-invalid': error ? true : undefined,
    'aria-describedby': describedBy,
  };

  return (
    <Field data-invalid={error ? true : undefined} className="gap-2">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      {multiline ? (
        <Textarea
          {...control}
          onChange={(event) => field.handleChange(event.target.value)}
          disabled={inputProps.disabled}
          required={inputProps.required}
        />
      ) : (
        <Input {...inputProps} {...control} onChange={(event) => field.handleChange(event.target.value)} />
      )}
      {hint ? <FieldDescription id={`${id}-hint`}>{hint}</FieldDescription> : null}
      {/* The line is always there: a message appearing when the person leaves the field must not
          push the submit button away from under their pointer. */}
      <div id={`${id}-error`} className="min-h-5">
        {error ? <FieldError>{error}</FieldError> : null}
      </div>
    </Field>
  );
}
