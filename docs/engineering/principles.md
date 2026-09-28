# Engineering principles

The rules every line of code follows, whatever the language or layer. The
backend and frontend guides add what is specific to each side.

The spirit: **senior, simple, correct.** Code a teammate understands in one
read, that does exactly what the story asks, that fails loudly and safely, and
that is easy to change next month. Not clever, not over-built.

---

## 1. Simplicity first (not overkill)

- Solve the problem at hand. No abstraction for a future that is not in a story.
- The simplest design that is **correct, secure and maintainable** wins. If two
  are equal, pick the one with fewer moving parts.
- **Rule of three:** duplicate once if you must; the third occurrence gets
  extracted. A premature helper with one caller is noise.
- No configuration nobody asked for, no plugin system for one plugin, no
  generic engine for one case.
- Before adding a dependency, check the platform or an existing dependency
  cannot do it. Every dependency is code you now maintain and patch.
- Delete more than you add when you can. Less code is fewer bugs.

## 2. DRY — one source of truth

- Every business rule, price, limit, label list, enum mapping or formatting rule
  lives in **one** place, and everything else reads it.
- Numbers shown to users (prices, limits, quotas) are **read from the same
  constants the code enforces**, never retyped in copy.
- Types that mirror another service are **generated** from its contract
  (OpenAPI, protobuf…), not hand-copied.
- DRY never justifies crossing a module boundary: shared code goes to the
  shared layer or behind the owning module's public API.

## 3. Naming and structure

- Names say what a thing *is* or *does* in the business language
  (`reserveSlot`, `TicketOrder`), not how it is built (`handleData`, `Manager2`).
- Functions do one thing. If you need "and" to describe it, split it.
- **Early returns** over nested conditions: handle the edge cases first, then
  the main path at the lowest indentation.
- Keep files focused. A file that grows past a few hundred lines is usually two
  concepts.
- Organize by **business domain** (vertical slices), not by technical kind:
  `booking/` holds its controller, service, repository, DTOs and tests together.
- Push conditionals to the edges: decide *which* implementation at the entry
  point (a factory, a strategy map), keep each implementation free of `if type ==`.

## 4. Comments and documentation

- Code says **what**; comments say **why**: a business rule, a non-obvious
  constraint, a workaround and its reason, a security consideration.
- Never restate the next line (`// get the user`).
- Public API (module interfaces, exported functions, endpoints) gets a short doc
  comment: what it guarantees, what it refuses, what it returns when empty.
- A decision with lasting impact goes into an ADR, not a comment.
- Documentation changes ship **with** the code that changes the behaviour
  (README, `.env.example`, story, ADR).
- Write in plain words: short sentences, active voice, concrete nouns. No
  marketing tone in code or docs.

## 5. Errors and failure

- **Fail fast and loudly** on programmer errors and missing configuration; never
  hide them behind a default.
- **Expected outcomes are not exceptions.** "Not found", "already taken",
  "limit reached" are normal results: return a typed result or a specific
  domain exception that maps to a precise status (404, 409, 422…).
- **Specific exceptions over generic ones.** `SlotUnavailableException`, not
  `RuntimeException("slot unavailable")`. Catch what you can handle, where you can
  handle it; let the rest reach the global handler.
- **Never swallow an error.** No empty `catch`. If you catch, you either
  recover meaningfully, translate to a domain error, or log with context and
  rethrow.
- **One global handler** turns unexpected errors into a clean 500 with a
  correlation id, reports them to error tracking, and **never leaks internals**
  (stack traces, SQL, class names) to the client.
- **User-facing messages** are written for people, translated, and say what to
  do next. Technical detail goes to logs, keyed by the correlation id.
- External calls (payment, messaging, storage) have timeouts, bounded retries
  for idempotent operations, and a clear failure path.

## 6. Configuration and secrets

- Every value that can differ between environments comes from an environment
  variable, read in one typed configuration object per concern.
- `.env.example` lists every variable with a comment and a safe placeholder.
  Local development works with no `.env` (sensible defaults for local only).
- Secrets never enter the repository, logs, error reports or client bundles.
- Required production values have no silent fallback: the app refuses to start.

## 7. Security by default

- Validate every input at the boundary (type, size, format, allowed values).
- Authorize every action on the server, even if the UI hides the button.
- Isolation between tenants/users is enforced in one central place (a filter, a
  policy), and tested. Never rely only on a hand-written `WHERE owner_id = ?`.
- Least privilege: tokens in httpOnly cookies, scoped links, short-lived codes.
- Uploaded files are checked by content (decode them), size and type.
- Rate-limit public endpoints. Log security-relevant events (who, what, when).
- Dependencies are scanned in CI; a high/critical fixable vulnerability fails
  the build.

## 8. Data and time

- Store timestamps in **UTC**. Carry an explicit IANA timezone for anything that
  happens somewhere (an event, a shop); render in that timezone, not the viewer's.
- Money is an integer in the currency's minor unit with its currency code; never
  a float.
- Schema changes are migrations: additive, forward-only, never edited once
  merged.

## 9. Performance, without premature optimization

- Avoid accidental O(n²): no lookup inside a loop over another list; use a map.
- Fetch only the fields you need; paginate lists; avoid N+1 queries.
- Measure before optimizing anything that is not obviously wrong.

## 10. Testing

- Tests prove the **acceptance criteria**, named after the behaviour they check
  (`a second organization cannot see the banner`).
- Test the edges the story names: limits, empty states, concurrency, isolation,
  time zones, the hour of the day.
- Unit tests for logic, integration tests against real infrastructure
  (containers) for persistence and wiring, a small end-to-end smoke suite for the
  journeys that must never break.
- A flaky test is a bug: fix the cause (clock, order, shared state), never retry
  it into green.
- Coverage is a floor (e.g. 70–80 % on business logic), not a goal.

## 11. Dead code and hygiene

- Delete unused code, config keys, translations, files and dependencies in the
  change that makes them unused.
- No commented-out code. Git remembers.
- No TODO without a ticket number.

## 12. Git hygiene

- Small, focused commits with Conventional Commit messages that explain why.
- Never rewrite shared history (`main`, `develop`, someone else's branch).
- Resolve conflicts by merging the base in; regenerate lockfiles and generated
  files with their tools, never by hand.
