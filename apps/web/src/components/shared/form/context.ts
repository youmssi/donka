import { createFormHookContexts } from '@tanstack/react-form';

/**
 * The contexts behind {@link useAppForm}: field components read their field with
 * `useFieldContext<T>()`, form components the form with `useFormContext()`.
 */
export const { fieldContext, formContext, useFieldContext, useFormContext } = createFormHookContexts();
