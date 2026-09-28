# Backend guide (Rust)

What is specific to `apps/app` and the `crates/`. Read `principles.md` first; this guide does
not repeat it. Practices taken from the upstream GoRules codebases are listed in `references.md`.

---

## 1. Architecture: a modular monolith of crates

- One deployable (`apps/app`), split into **business modules**, **one crate each**:
  `identity` (users, sessions), `project` (projects, members, decisions, versions), `release`
  (releases, environments, deployments, artifacts), `approval`, `audit`, `decision-log`,
  `explain`. A module owns its tables, types, queries and rules.
- A module is used by others **only** through its public API: the functions and types exported
  from its `lib.rs`, or domain events it publishes. Everything else is `pub(crate)`.
- **Boundaries are enforced by the compiler.** A crate can only use what another crate exports,
  and `Cargo.toml` lists the allowed dependencies. Cycles do not compile. Adding a dependency
  between two modules is a reviewed change.
- Never query another module's tables. Never join across two modules' tables in SQL.
- `crates/engine` is the only crate that imports `zen_engine` (ADR-003). `crates/shared` (when
  needed) holds only cross-cutting code: the error model, pagination, clock, ids.
- Split into separate services only when a real constraint forces it. Donka Runtime is already
  separate because it scales with customer traffic (ADR-001).

Inside a module, the request flow is:

```
Handler (Axum, in apps/app)  →  Service (use case, transaction)  →  Repository (sqlx queries)
       ↓                               ↓
 Request/Response DTOs          Domain types & rules
```

- **Handler:** extract, validate, authorize (extractor), call one service function, map the
  result to a response DTO. No business logic, no SQL.
- **Service:** one public function per use case. Owns the transaction (`&mut Transaction`).
  Enforces business rules. Publishes events.
- **Repository:** data access only. Names describe the data (`find_by_project`).
- Dependencies are passed in (`AppState`, function parameters, trait objects), never created
  inside a function and never global singletons. That keeps everything testable.

## 2. API conventions

- Every endpoint lives under one **versioned base path read from configuration**
  (`DONKA_API_BASE_PATH`, default `/api/v1`), applied once with `Router::nest`. Handlers declare
  only their resource path.
- **Additive changes only** on a published version.
- Resources are nouns, plural, kebab-case (`/projects/{id}/test-scenarios`). Non-CRUD actions are
  explicit sub-resources (`POST /releases/{id}/approve`).
- **DTOs at every boundary.** Never serialize a database row type directly; never accept one as
  input.
- **Validate at the edge** (sizes, formats, enums) with `validator` or explicit checks on the
  request DTO. The service still checks business invariants.
- Status codes: `201` created, `202` accepted, `204` no body, `400` invalid input (field errors),
  `422` business rule, `401`/`403`, `404` not found **or not yours**, `409` conflict, `429` rate
  limited, `500` with a request id.
- Lists are paginated with a hard maximum page size.
- The OpenAPI contract is **generated from the code** with `utoipa` and published at
  `/api/v1/openapi.json`; the web app generates its types from it.
- Times are ISO-8601 UTC.

## 3. Error model

One shape for the whole API:

```json
{ "message": "You cannot approve a release you authored.", "code": "SELF_APPROVAL",
  "requestId": "3f1c…", "fields": { "email": "must be a valid email" } }
```

- Each module has its own error enum (`thiserror`). Expected outcomes are variants
  (`ReleaseNotFound`, `SelfApproval`, `AlreadyApproved`), never a generic string error.
- `apps/app` has **one** `IntoResponse` mapping from module errors to status + code + safe
  message. Unexpected errors are logged with the request id and returned as a generic `500`: no
  SQL, no Rust type names, no input echo.
- Never discard an error (`let _ = …` on a `Result`, `.ok()` to silence it) unless a comment says
  why it is safe.
- No `unwrap()`/`expect()` outside tests and startup code that must fail fast.

## 4. Persistence

- PostgreSQL 16, `sqlx` with compile-time-checked queries where practical.
- **Migrations** in `migrations/`: versioned, additive, forward-only. A merged migration is never
  edited. Destructive changes go expand → migrate data → contract across releases.
- Constraints live in the database: `NOT NULL`, foreign keys, unique indexes, checks.
- **Append-only tables** (`audit_events`, `decision_log`): a trigger rejects `UPDATE` and
  `DELETE`, and the application role is not granted those privileges.
- **Concurrency:** "only one can win" rules (one approval per release, one active deployment per
  environment) use a unique constraint or a row version. Test with two concurrent requests.
- **Isolation.** Donka runs one installation per customer, so there are no tenants. The boundary
  is the **project**: every project-scoped route uses one `ProjectAccess` extractor that loads the
  caller's membership and role, and returns `404` when the caller is not a member. Every
  project-scoped feature has a test proving a member of project A cannot read or change project B.
- Money stored by Donka: integer minor units + ISO currency code. Applicant data inside decision
  inputs belongs to the customer's rules and is evaluated with exact decimals by the engine.

## 5. Transactions, events and side effects

- The **service function is the transaction boundary**.
- Side effects that leave the system (email, webhook, publishing an artifact to storage) run
  **after commit**, through an outbox table processed by a background worker, so a rollback
  never publishes a release that was not recorded.
- Handlers of events and outbox jobs are **idempotent** (dedupe key or state check).

## 6. External providers

- Object storage, SMTP, the customer's LLM endpoint and bureau/KYC connectors each sit **behind a
  trait** owned by the module that needs it; the implementation is chosen by configuration.
  Tests use a fake.
- Every call has a **timeout** and bounded **retries with backoff** for idempotent operations.
  Retry only transport errors, `408`, `429` and `5xx`: any other `4xx` is an answer, not a blip.
- Provider errors are translated into the module's own errors; provider types never leak out.

## 7. Configuration

- One typed `Config` per concern, read from environment variables (`DONKA_*`) at startup and
  validated. `.env.example` lists every variable.
- Local development works with no `.env`. Secrets (`DONKA_SESSION_SECRET`, storage keys…) have no
  production fallback: the app refuses to start without them when `DONKA_ENV=production`.
- Variable names match `docker-compose.yml` so one `.env` drives both.

## 8. Security

- Studio users: session cookie **httpOnly, Secure, SameSite=Lax**, served from the same origin as
  the web app (ADR-002). No tokens in browser storage.
- Authorization on every endpoint, deny by default: routes are unreachable without the session
  extractor, project routes require `ProjectAccess` with a minimum role.
- Passwords with argon2id; reset and invite tokens single-use, hashed at rest, short-lived.
- Runtime access tokens: random, shown once, stored hashed, scoped to one environment.
- Tokens and connection strings never appear in logs, error bodies or CI output.
- Rate limiting on login, password reset and simulate.
- CORS off by default (same origin); allowed origins only from configuration.
- `cargo audit` (RustSec) in CI; a fixable high/critical advisory fails the build.

## 9. Observability

- A **request id** on every request (`x-request-id` in or generated), in the tracing span and in
  error bodies.
- Structured JSON logs in production (`DONKA_LOG_FORMAT=json`): level, message, request id, user
  id, project id. Never passwords, tokens, or applicant data.
- Levels: `ERROR` someone must act, `WARN` unexpected but handled, `INFO` business milestones
  (release approved, deployed), `DEBUG` off in production.
- Health: `GET /api/v1/health` (liveness) and `GET /api/v1/ready` (database reachable).

## 10. Testing

| Kind | What | Tooling |
|---|---|---|
| Unit | Domain rules, mappers | `#[test]`, `#[tokio::test]` |
| Integration | Repositories, migrations, transactions, full HTTP slice | real PostgreSQL (`DATABASE_URL`; CI service container), `sqlx::test` |
| Boundaries | Module dependencies | the compiler (crate graph) |
| Contract | OpenAPI generated and valid | build step |
| Smoke | Health + simulate against a deployed stack | script in CI after deploy |

- Integration tests use a real PostgreSQL, never a substitute.
- Tests do not depend on the wall clock, the timezone or execution order: inject a `Clock`.
- Coverage (`cargo llvm-cov`) floor of 70 % on business crates, enforced in CI once the first
  module lands.

## 11. CI

In order, each step blocking: format, clippy, build + tests (with PostgreSQL), engine version
check, coverage gate, `cargo audit`, container image build.

## 12. Checklist for a new endpoint

- [ ] Resource path only; base path from config
- [ ] Request DTO validated; response DTO exposes only what is needed
- [ ] Authorization declared (session / `ProjectAccess` + role)
- [ ] Expected failures mapped to specific statuses and codes
- [ ] Transaction in the service; side effects through the outbox
- [ ] Pagination for lists
- [ ] Integration test: happy path, validation error, forbidden, other project, conflict/limit
- [ ] OpenAPI regenerated; web types regenerated in the same PR
- [ ] New variables in `.env.example`
