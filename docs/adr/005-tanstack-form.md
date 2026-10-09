# ADR-005: TanStack Form + Zod for all forms

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-1

## Context

The playbook asks for one form stack (its example is react-hook-form). Fieldkit uses TanStack
Form because it is framework-agnostic.

## Decision

TanStack Form + Zod in Studio as well, so all Donka products share one form engine and the same
field patterns.

## Consequences

- Shared field components can move between Studio and Fieldkit.
- shadcn's default `Form` wrapper (react-hook-form) is not used; Studio has its own small field
  components on top of shadcn inputs.
- Since DNK-44, Studio wraps TanStack Form once, as its guides recommend: `createFormHook` in
  `components/shared/form` gives every form `useAppForm`, with the field components (`TextField`,
  `SelectField`, `CheckboxField`) and form components (`Form`, `SubmitButton`) bound to it. A field
  component reads its field from context (`useFieldContext<T>()`) rather than taking an untyped
  field as a prop. This is the set Fieldkit will reuse when it draws a form from an input
  contract: it picks a component per field from the contract's type and widget.
