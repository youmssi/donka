import { createFormHook } from '@tanstack/react-form';

import { CheckboxField } from './checkbox-field';
import { fieldContext, formContext } from './context';
import { Form } from './form';
import { SelectField } from './select-field';
import { SubmitButton } from './submit-button';
import { TextField } from './text-field';

/**
 * Studio's form hook (ADR-005). Every form calls `useAppForm` and renders its fields with
 * `form.AppField`, e.g. `<form.AppField name="name">{(field) => <field.TextField label="…" />}</form.AppField>`,
 * inside `<form.AppForm><form.Form>…<form.SubmitButton /></form.Form></form.AppForm>`.
 * A large form is split into parts with `withForm`.
 */
export const { useAppForm, withForm } = createFormHook({
  fieldContext,
  formContext,
  fieldComponents: { TextField, SelectField, CheckboxField },
  formComponents: { Form, SubmitButton },
});
