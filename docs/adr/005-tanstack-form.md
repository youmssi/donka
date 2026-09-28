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
