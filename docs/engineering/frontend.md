# Frontend guide (apps/web)

What is specific to the Studio web app. Read `principles.md` first. Stack: Next.js (App Router,
**static export**), React, TypeScript, Tailwind, shadcn/ui on Radix, TanStack Form + Zod,
TanStack Query, next-intl, `@gorules/jdm-editor`.

---

## 1. Architecture: modules by business domain

```
apps/web/src/
├── app/[locale]/…            # routing only (static export)
├── components/ui/            # shadcn/ui components from the registry, as published; no business logic
├── components/shared/        # cross-cutting helpers and types, used by 2+ modules
├── modules/workspace/        # the signed-in shell: sidebar, breadcrumb, command menu (composes modules)
├── modules/<domain>/         # identity · project · decision · release · approval · audit · decision-log
│   ├── schema.ts             # CONTRACT  — generated API types + Zod schemas
│   ├── <domain>.service.ts   # SERVICE   — all calls for the domain; the ONLY place that looks at HTTP statuses
│   ├── use<Domain>.ts        # CACHE     — TanStack Query hooks (optional)
│   ├── <feature>.tsx         # COMPONENT — UI + form validation, never raw fetch
│   └── index.ts              # BARREL    — the module's public surface
└── messages/{en,fr}.json     # every user-facing string
```

Dependencies go **one way only**: `app/ → Component → Cache hook → Service → Contract`.

- **Routing (`app/`)**: reads params and search params, guards, metadata; renders one top-level
  component imported **from the module barrel**. No business or presentation logic in pages.
  When a page shows two modules (a project's settings with the decision log's), a small client
  component next to the page composes them through a slot (`ProjectSettingsPage more={…}`), so
  neither module imports the other.
- **Component**: renders and validates. Never calls `fetch`.
- **Cache hook**: TanStack Query for data that changes after load (release status, decision log).
- **Service**: HTTP calls to Studio's API; converts every response into an `ActionResult`.
- **Contract**: types **generated** from `/api/v1/openapi.json` (`openapi-typescript`), never
  hand-copied.
- Other code imports a module only through its `index.ts`. Enforced by ESLint
  (`no-restricted-imports` / `import/no-internal-modules`).

## 2. Rendering (ADR-002)

- Studio is built with `output: 'export'` and served by `apps/app` from the same origin. There is
  no Node server in a customer installation.
- Consequences we accept:
  - Pages render on the client after a static shell. Components that need hooks are client
    components; that is the norm here, not an exception.
  - **No runtime dynamic segments.** Records are addressed with search params
    (`/en/projects?id=…`, `/en/editor?project=…&decision=…`). Locale segments are pre-generated
    with `generateStaticParams` (`en`, `fr`).
  - No server actions or route handlers. The security goal of a BFF is met by the same-origin
    httpOnly session cookie set by `apps/app` (§3).
- `@gorules/jdm-editor` (antd + WASM) is loaded with `next/dynamic` and `ssr: false`, only on the
  editor page (ADR-004). It is used unchanged, extended only through its props:
  - its theme comes from Studio's tokens, read at runtime and converted from `oklch()`
    (`modules/decision/jdm-graph.tsx`);
  - the node that calls another decision (`decisionNode`) is given through `components`, with the
    project's decisions to choose from;
  - the connector node (`customNode` of kind `donka.connector`) is given through `customNodes`.
    The editor only opens tabs for `components`, so its settings open in a shadcn `Sheet` with a
    TanStack Form whose Zod rules mirror the Runtime's (`modules/decision/connector.ts`);
  - a pnpm override lifts its antd to the last 5.x release, which has the official React 19 patch.
- **Nothing loads from a CDN at runtime.** A self-hosted installation runs in networks that block
  the internet, and code must come from the image. Monaco (the editor's code panels) defaults to
  a CDN; `monaco-setup.ts` bundles it and its workers instead. Check any new library for this.

## 3. Talking to the backend

- The browser **never holds auth tokens**. `apps/app` sets an httpOnly, Secure, SameSite=Lax
  session cookie; requests are same-origin, so the cookie travels automatically. State-changing
  requests carry the CSRF header the API requires.
- One HTTP client (`ky`), configured once: base path `/api/v1`, timeout, `x-request-id`.
- Every service call returns a **discriminated union**, never throws for an expected outcome
  (`components/shared/api`):

  ```ts
  export type ActionResult<T> = { ok: true; data: T } | { ok: false; error: ActionError };
  interface ActionError { code: ErrorCode; requestId?: string; fieldErrors?: Record<string, string> }
  ```

- The service is the only layer that inspects status codes. It maps every failure to a code the
  web app knows (`KNOWN_ERROR_CODES`; anything else becomes `UNEXPECTED`, an unreachable server
  `NETWORK`), and the component shows `errors.<code>` from the catalogs through `ErrorAlert`.
  Services stay free of React and locale state; messages stay in one place. Only failures on our
  side (`UNEXPECTED`, `DATABASE_UNAVAILABLE`) show the request id to quote.
- Contract values the UI must agree with (password length…) are read from `openapi.json`, not
  copied: see `modules/identity/schema.ts`.

## 4. Forms

- **TanStack Form + Zod** for every form (ADR-005), rendered with the shadcn `Field` family
  (`FieldGroup`, `Field`, `FieldLabel`, `FieldDescription`, `FieldError`) as in shadcn's
  TanStack Form guide. Small wrappers that bind a TanStack field to them live in
  `components/shared/form/`; no form builds its own label/error markup.
- The Zod schema is the single source of the form's rules; the server validates again.
- Errors next to the field, in plain language, once the person has left it or submitted; a
  corrected value clears its error at once. Forms validate on `onChange` and `onSubmit` (an
  `onBlur` validator would keep a stale error until the next blur); the field shows errors only
  once left or submitted, so nobody is told off while still typing. The line under each field is
  reserved, so an error that appears when the person leaves the last field does not
  move the submit button out from under their pointer (a missed click).
- Submit disabled while pending, with a `Spinner` in the button; no double submission; never
  wipe what the user typed on error. A success that leaves the screen as it was is confirmed by a
  toast (`sonner`); an error stays on the screen, next to what caused it.

## 5. UI and UX

Studio is a desktop-first work tool (analysts at a desk), but every screen must remain usable at
390 px for approvals and decision lookups on a phone.

- **One primary action per screen.** Destructive or irreversible actions (deploy to production,
  approve, rollback) ask for confirmation that names what will happen.
- **Short labels** ("Request approval", "Deploy to staging").
- **Every data view has four states:** loading (skeleton), empty (says what to do next, with the
  action), error (what happened, how to retry), success.
- **Design tokens** (colours, radius, fonts) are CSS variables shared by the shadcn components and
  the antd theme used inside jdm-editor, so the editor matches the shell.
- **Components come from the shadcn registry** (new-york-v4 sources in `components/ui/`, taken
  as published; only import paths change, and a component gets `asChild` where it must wrap the
  locale-aware `Link`). Check the registry before writing markup: a need
  already covered by a component is built with it. The map below is the default per need:

  | Need | Component |
  |---|---|
  | Navigation, sections of the open project | `Sidebar` (+ `Collapsible`), `Breadcrumb` in the header |
  | Jump anywhere | `Command` dialog (Ctrl/⌘ K) with `Kbd` hints |
  | Any list of records | data table: `Table` + TanStack Table (`components/shared/data-table`) |
  | Nothing to show | `Empty` (icon, title, what to do next, the action) |
  | Loading | `Skeleton` shaped like the content; `Spinner` inside a pending button |
  | Row actions | `DropdownMenu` on a `…` icon button; destructive ones confirmed by `AlertDialog` |
  | Create or edit in place | `Dialog` holding a form |
  | Switch between views of one list | `Tabs` or `ToggleGroup` |
  | Pick a date range | `Popover` + `Calendar` (`mode="range"`) |
  | Pick among many (people) | `Popover` + `Command` (combobox) |
  | Short confirmation | `sonner` toast |
  | Icon-only button | `Button size="icon-sm"` + `Tooltip` with its name |
  | Short list inside a card, sheet or dialog (tokens, frozen versions, changes) | `ItemGroup` + `Item` (`ItemSeparator` between, `ItemActions` for buttons) |
  | Something that needs attention (in progress, waiting, failed) | `Alert` (default or `destructive`, `Spinner` as its icon while in progress) |
  | Form label outside a TanStack form | `Field` + `FieldLabel` |
  | Value to copy | `InputGroup` with an `InputGroupButton` |
  | Previous / next pages | `Pagination` (`components/shared/pager`) |

- **Dates and times** go through `components/shared/format` only: "13 Nov" in the current year,
  "13 Nov 2025" otherwise, times "14:05" in the viewer's locale and time zone, the full date and
  time in a tooltip, relative ("2 h ago") only where recency matters (audit, activity).
- **Links are short and readable:** a project is addressed by its key (`?p=credit-pme`), never by
  an id in what a person sees or shares.
- **Use the width.** Pages fill the space next to the sidebar; tables are compact; no decorative
  description where the title says it.
- **Verify visually** before a UI PR is ready: desktop and 390 px, English and French, light and
  dark; attach screenshots.

## 6. Accessibility (not optional)

Semantic HTML, labelled inputs, alt text, keyboard access with visible focus, dialogs that trap
and return focus (Radix does this), WCAG AA contrast, colour never the only signal (release
status uses text + icon + colour), `prefers-reduced-motion`, `<html lang>` per locale.

## 7. Internationalization

- Every user-facing string lives in `messages/en.json` and `messages/fr.json`. English is the
  reference locale; `pnpm i18n:check` fails on missing or extra keys.
- Dates, numbers and money through `Intl` with the active locale. Times shown in the viewer's
  configured timezone, labelled.
- next-intl navigation helpers keep the locale; the language switch links to the locale-neutral
  path with an explicit target locale.

## 8. Performance

- The editor bundle (jdm-editor, antd, WASM) loads only on the editor page.
- No request waterfalls: parallel queries; paginate lists; virtualize long tables (decision log).
- Fonts self-hosted through `next/font`.

## 9. SEO

Studio is a signed-in tool: `robots` disallows everything; no sitemap. Titles are still unique
per page for browser tabs and history.

## 10. TypeScript and code style

`strict: true` and `noUncheckedIndexedAccess: true` (index access returns `T | undefined`), no `any`, no unexplained `!`. `interface` for object shapes, `type` for unions.
Branded ids (`ProjectId`, `ReleaseId`). `function` declarations for components. Path alias `@/`.
Prettier (`printWidth: 120`, `singleQuote: true`, as in donka-cli) and ESLint settle style.

## 11. Tooling notes

- shadcn components live in `components/ui` and follow the new-york v4 sources on the `radix-ui`
  package; `components.json` makes `pnpm shadcn add <name>` work (the `shadcn` CLI is a dev
  dependency, so everyone runs the same version).
- `apps/web/.mcp.json` declares the shadcn MCP server (`shadcn mcp`), so coding assistants
  started in `apps/web` can browse and add registry components. It reaches `ui.shadcn.com`.
- Next.js 16 ships its own docs in `node_modules/next/dist/docs/`: read them before relying on
  memory. `next dev` writes `AGENTS.md`/`CLAUDE.md` into `apps/web` when it detects a coding
  agent; they are ignored by git.

## 12. Configuration

The static bundle contains no secrets and no environment-specific URLs: it calls the same origin.
Build-time `NEXT_PUBLIC_*` values are limited to non-secret flags and are listed in
`.env.example`.

## 13. Testing

| Kind | What | Tooling |
|---|---|---|
| Unit | Helpers, formatters, schema rules, service status mapping | Vitest |
| Component | Forms and the four data states | Testing Library |
| i18n | Catalog parity | `pnpm i18n:check` |
| End-to-end | Sign in → edit → simulate → release → approve → deploy, desktop and phone | Playwright |

E2E tests wait for real conditions, never fixed sleeps, and create their own data.

## 14. Checklist for a new screen

- [ ] Page is thin; renders a component from the module barrel
- [ ] Data through the service; `ActionResult` handled; no raw fetch
- [ ] Loading, empty, error and success states
- [ ] All strings in English and French; dates and numbers via `Intl`
- [ ] Keyboard, labels, alt text, contrast checked
- [ ] Checked at 390 px and desktop, both languages, light and dark; screenshots in the PR
