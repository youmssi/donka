'use client';

import type { AnyFieldApi } from '@tanstack/react-form';
import { useTranslations } from 'next-intl';
import { useId, type ComponentProps } from 'react';

import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';

interface TextFieldProps extends Omit<ComponentProps<typeof Input>, 'name' | 'value' | 'onChange' | 'onBlur'> {
  field: AnyFieldApi;
  label: string;
  hint?: string;
  /** Values for placeholders in validation messages, e.g. `{ min: 12 }`. */
  messageValues?: Record<string, number | string>;
  /** Message key (under `validation`) sent back by the server for this field. */
  serverError?: string;
  /** A text area instead of a one-line input. */
  multiline?: boolean;
}

function firstMessage(errors: unknown[]): string | undefined {
  for (const error of errors) {
    if (typeof error === 'string') return error;
    if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') {
      return error.message;
    }
  }
  return undefined;
}

/**
 * Label, input, hint and error for a TanStack Form field. Errors appear after the
 * person leaves the field or submits, never while they are still typing.
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
  const describedBy = [hint ? `${id}-hint` : null, error ? `${id}-error` : null].filter(Boolean).join(' ') || undefined;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {multiline ? (
        <Textarea
          id={id}
          name={field.name}
          value={String(field.state.value ?? '')}
          onChange={(event) => field.handleChange(event.target.value)}
          onBlur={field.handleBlur}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          disabled={inputProps.disabled}
          required={inputProps.required}
        />
      ) : (
        <Input
          id={id}
          name={field.name}
          value={String(field.state.value ?? '')}
          onChange={(event) => field.handleChange(event.target.value)}
          onBlur={field.handleBlur}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          {...inputProps}
        />
      )}
      {hint ? (
        <p id={`${id}-hint`} className="text-xs text-muted-foreground">
          {hint}
        </p>
      ) : null}
      {/* Always rendered with room for one line: a message appearing when the person leaves the
          field must not push the submit button away from under their pointer. */}
      <p id={`${id}-error`} className="min-h-5 text-sm text-destructive">
        {error}
      </p>
    </div>
  );
}
