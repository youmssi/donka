'use client';

import { Eye, EyeOff } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useState, type ComponentProps } from 'react';

import { cn } from '@/components/shared/utils';
import { Field, FieldDescription, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group';
import { Textarea } from '@/components/ui/textarea';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

import { useFieldContext } from './context';

interface TextFieldProps extends Omit<ComponentProps<typeof Input>, 'name' | 'value' | 'onChange' | 'onBlur'> {
  label: string;
  hint?: string;
  /** Values for placeholders in validation messages, e.g. `{ min: 12 }`. */
  messageValues?: Record<string, number | string>;
  /** Message sent back by the server for this field, already translated. */
  serverError?: string;
  /** A text area instead of a one-line input, this many lines high. */
  multiline?: boolean;
  rows?: number;
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

/** The message key shown under a field: only once the person has left it or submitted the form. */
export function shownError(meta: { isBlurred: boolean; errors: unknown[] }, submissionAttempts: number) {
  return meta.isBlurred || submissionAttempts > 0 ? firstMessage(meta.errors) : undefined;
}

/**
 * A text input bound to the surrounding `form.AppField`, laid out with shadcn `Field`.
 * Errors appear after the person leaves the field or submits, never while they
 * are still typing.
 */
export function TextField({ label, hint, messageValues, serverError, multiline, rows, ...inputProps }: TextFieldProps) {
  const field = useFieldContext<string>();
  const t = useTranslations('validation');
  const common = useTranslations('common');
  const [passwordVisible, setPasswordVisible] = useState(false);
  const toggleLabel = common(passwordVisible ? 'hidePassword' : 'showPassword');
  const id = useId();
  const key = shownError(field.state.meta, field.form.state.submissionAttempts);
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
          className={inputProps.className}
          spellCheck={inputProps.spellCheck}
          placeholder={inputProps.placeholder}
          rows={rows}
        />
      ) : inputProps.type === 'password' ? (
        <InputGroup data-disabled={inputProps.disabled || undefined}>
          {/* Edge's native reveal button would duplicate the shared control. */}
          <InputGroupInput
            {...inputProps}
            {...control}
            className={cn(inputProps.className, '[&::-ms-reveal]:hidden')}
            type={passwordVisible ? 'text' : 'password'}
            onChange={(event) => field.handleChange(event.target.value)}
          />
          <InputGroupAddon align="inline-end">
            <Tooltip>
              <TooltipTrigger asChild>
                <InputGroupButton
                  type="button"
                  size="icon-sm"
                  aria-label={`${toggleLabel}: ${label}`}
                  aria-controls={id}
                  disabled={inputProps.disabled}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => setPasswordVisible((visible) => !visible)}
                >
                  {passwordVisible ? <EyeOff aria-hidden /> : <Eye aria-hidden />}
                </InputGroupButton>
              </TooltipTrigger>
              <TooltipContent>{toggleLabel}</TooltipContent>
            </Tooltip>
          </InputGroupAddon>
        </InputGroup>
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
