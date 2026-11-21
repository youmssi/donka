# ADR-004: shadcn/ui on Radix for Studio; jdm-editor used unchanged

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-1

## Context

`@gorules/jdm-editor` (the rule editor) is built on antd 5: 60 of its source files import antd.
Fieldkit, Donka's open-source form toolkit, is built on shadcn/ui.

## Options considered

1. **shadcn/ui (Radix) shell + jdm-editor unchanged**, antd themed with the same tokens.
2. **Fork jdm-editor and port it to shadcn.** One design system; weeks of work before the MVP and
   every upstream change becomes a merge conflict.
3. **antd everywhere.** Fast, but Studio looks like GoRules and shares nothing with Fieldkit.
4. **shadcn on Base UI** instead of Radix. Good, newer; smaller ecosystem than Radix today.

## Decision

Option 1 with Radix.

## Consequences

- antd is loaded only on the editor page; the rest of Studio is pure shadcn/Radix.
- Design tokens (CSS variables) drive both shadcn and the antd `ConfigProvider` theme.
- Revisit after the pilot if customers notice the editor looks different: port dialogs and menus
  first, the decision table last.
