# Component previews

Storybook renders the real shared components with Studio's styles, local Geist fonts,
English/French catalogs and application providers. It is a development tool; the Next.js
static export does not include it. No Studio server, database or credentials are needed.

## Run and verify

From the repository root, after installing the web dependencies:

```bash
pnpm --dir apps/web storybook
```

Open `http://127.0.0.1:6006`. The toolbar selects English/French and light/dark. Stories
are typed CSF files in `apps/web/src/stories`, outside the registry sources in `components/ui`.
The Next.js Vite framework provides the App Router navigation mocks. Navigation links are
examples; use the locale toolbar to change the preview language.

For the same checks as CI:

```bash
pnpm --dir apps/web build-storybook
pnpm --dir apps/web exec playwright install --with-deps --only-shell chromium
pnpm --dir apps/web test:storybook
```

The browser check serves `storybook-static` on an ephemeral loopback port and opens every
story at 1280 px and 390 px, in both languages and both themes. A rendering error, console
error, wrong document language/theme or horizontal document overflow fails the check.
It also exercises password reveal, form validation/submission, clipboard failure, overlays,
theme changes, the mobile sidebar and tour replay. CI builds and tests the previews in the
web job, then keeps the static Storybook artifact for seven days.

For an installed browser, set `DONKA_TEST_BROWSER_CHANNEL` (for example, `msedge`). To test
an existing build elsewhere, set `DONKA_STORYBOOK_DIR` to its directory. Both are optional,
Node-only settings documented in `.env.example`.

## Coverage

Stories compose related primitives the way application screens do. Loading, empty, error
and success are separate stories where data can have those states. Static branding and
controls instead show their relevant variants, disabled states and interactions.

| Story group | Shared components and registry families | States or examples |
|---|---|---|
| Shared/Data table | DataTable, Table, Skeleton, Pager, Empty, ErrorAlert | Loading, empty, error, success |
| Shared/Form | useAppForm, Form, TextField, SelectField, CheckboxField, SubmitButton; Field, Input, Textarea, Select, Checkbox | Pending submission, untouched, validation errors, saved |
| Shared/Password | TextField; InputGroup, Button, Tooltip | Paired masked fields, reveal/hide, disabled |
| Shared/Feedback | PageSkeleton, ErrorAlert; Alert, Badge, Empty, Spinner | Loading, empty, unexpected error with reference, success, rate limit, invalid pack |
| Shared/Dates | DateRangeFilter, When; Calendar, Popover | Empty/selected range, calendar, date/time/relative formats |
| Shared/Layout | BrandMark, Brand, AppShell, CenteredColumn, PageHeader, LanguageSwitch, ThemeToggle | Branding/preferences, heading, public shell |
| Shared/Tour | TourProvider, usePageTour, useCurrentTour | Replayable two-step tour |
| UI/Actions | Button, RadioGroup, Label, Spinner, Sonner | Variants, pending, disabled, radio choices, success/error toasts |
| UI/Content | Person, ShownOnceToken; Avatar, Card, Item, Badge | Card, list, long email/fallback avatar, token, clipboard failure |
| UI/Navigation | Pager; Breadcrumb, Tabs, Separator, Command, Kbd, DropdownMenu, Pagination | Tabs, command search, menu, pagination |
| UI/Overlays | Dialog, AlertDialog, Sheet, Popover, Tooltip | Open, dismiss and confirmation controls |
| UI/Sidebar | Sidebar, SidebarProvider, SidebarInset, Brand, Person | Loaded menu, skeleton, desktop collapse/mobile sheet |

Providers, hooks, API clients, types and utility functions without their own visual output
are exercised by their consumers or unit tests; they do not get artificial display components.

## Add or change a component

- Reuse the real shared component, its public types and existing composition patterns.
- Use the catalogs through `useStoryMessages`; keep fixture values synthetic and local.
- Keep fixtures deterministic. Never call a business API or use real emails, tokens or keys.
- Cover applicable data states and failure paths. Use real TanStack Form validation and
  submission state rather than imitating an invalid control with CSS.
- Keep state inside a preview component; reset it when its scenario changes. Effects that
  override a browser API for an error example must restore it on cleanup.
- Update the coverage table and add a browser assertion when a regression needs interaction
  to reveal it. Inspect desktop/390 px, English/French, light/dark and attach PR screenshots.

The setup follows the typed stories in upstream `jdm-editor`, with the current
[Next.js Vite framework](https://storybook.js.org/docs/get-started/frameworks/nextjs-vite).
The preview bundles the same installed Geist font assets directly because Vite's Next font
transform does not process the font-loader calls inside that package's published files.
