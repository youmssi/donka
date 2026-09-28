# Stage 1 — pilot-ready (stories)

Order of delivery is in `docs/ROADMAP.md`. Repos: **S** = donka (Studio), **R** = donka-runtime,
**C** = donka-cli. Stories spanning repos use the same branch name in each (producer merges
first). Every story also carries the standing criteria: checks green, docs and `.env.example`
updated, EN + FR strings for UI, screenshots for UI.

---

### DNK-1 — Work the same way in every Donka repo

**Type:** chore · **Repos:** S, R, C · **Dependencies:** none · **Size:** S

#### Why
Three repos, one team: without shared rules each repo drifts in branching, commits and quality bar.

#### Behaviour
| Where | After |
|---|---|
| All repos | `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, PR template, `develop` branch |
| Studio | Engineering guides adapted to Rust + static Next.js, ADR-001…006, this backlog |

#### Acceptance criteria
- [ ] Each repo has the playbook files with no placeholder left
- [ ] Studio guides state every deviation from the generic playbook and link the ADR behind it
- [ ] The forks state how they follow the playbook without restructuring upstream code (ADR-006)
- [ ] Repository settings to apply by hand are listed in `CONTRIBUTING.md`

#### Out of scope
Code changes (DNK-2 onward).

---

### DNK-2 — API conventions every endpoint follows

**Type:** refactor · **Repos:** S · **Dependencies:** DNK-1 · **Size:** S

#### Why
Every later endpoint copies the first ones; the base path, error shape and request id must be right
before there are thirty endpoints.

#### Behaviour
| Where | Before | After |
|---|---|---|
| All routes | `/api/…` hardcoded | `/api/v1/…` from `DONKA_API_BASE_PATH` |
| Errors | ad-hoc JSON | `{ message, code, requestId, fields? }` |
| Every response | — | `x-request-id` header (incoming one reused) |
| Contract | — | `GET /api/v1/openapi.json` generated with `utoipa` |

#### Acceptance criteria
- [ ] Changing `DONKA_API_BASE_PATH` moves every route; no handler contains the version
- [ ] Simulate errors keep their meaning with the new shape: unknown decision `404 DECISION_NOT_FOUND`, invalid model `422 INVALID_DECISION`, engine rejection `422 EVALUATION_FAILED`
- [ ] An unexpected failure returns `500 INTERNAL_ERROR` with a request id and no internal detail; the log line carries the same id
- [ ] `GET /health` (liveness) answers without the database
- [ ] Invalid configuration stops startup with a message naming the variable

#### Out of scope
Database readiness (DNK-3), authentication (DNK-4).

---

### DNK-3 — Persistent storage for Studio

**Type:** feature · **Repos:** S · **Dependencies:** DNK-2 · **Size:** M

#### Why
Nothing is stored today. Every following feature needs PostgreSQL, migrations and a tested way to
run them in CI.

#### Acceptance criteria
- [ ] `DATABASE_URL` required; Studio applies pending migrations at startup (can be disabled with `DONKA_DB_MIGRATE=false`)
- [ ] `GET /ready` returns `503` when the database is unreachable, `200` otherwise
- [ ] CI runs integration tests against PostgreSQL 16
- [ ] A reusable append-only guard (trigger rejecting `UPDATE`/`DELETE`) exists, with a test that proves it
- [ ] Shared persistence crate (`crates/db`: pool, migrations, readiness); business-module crates arrive with their first table (`crates/identity` in DNK-4), so no empty crate is created ahead of use

#### Out of scope
Business tables other than what DNK-4 needs.

---

### DNK-4 — Sign in to Studio

**Type:** feature · **Repos:** S · **Dependencies:** DNK-3 · **Size:** M

#### Why
Studio holds credit policy. Only known people may see or change it, and every change must be
attributable.

#### Decision
Session cookie (httpOnly, Secure, SameSite=Lax) on the same origin as the web app (ADR-002),
sessions stored in PostgreSQL as token hashes on Studio's own pool (ADR-007), argon2id password
hashes, CSRF enforced with the `x-donka-csrf` header. A locked account gets the same
`INVALID_CREDENTIALS` answer as a wrong password, so lockout does not reveal that an account exists. First administrator created from
`DONKA_BOOTSTRAP_ADMIN_EMAIL` on an empty database, with a one-time password-setup link printed to
the log.

#### Acceptance criteria
- [x] Sign in with email + password; wrong credentials return one generic message (no account enumeration)
- [x] Sign out invalidates the session server-side
- [x] Every route except health, ready, sign-in and password setup returns `401` without a session
- [x] State-changing requests without the CSRF header are refused (`403`)
- [x] Five failed sign-ins for one account in 15 minutes lock it for 15 minutes (limits from config)
- [x] Sessions expire after `DONKA_SESSION_IDLE_MINUTES` of inactivity
- [x] Passwords are never logged or returned

#### Out of scope
SSO/OIDC (Stage 3). Password reset and invitations (DNK-5).

---

### DNK-5 — Invite teammates and reset passwords

**Type:** feature · **Repos:** S · **Dependencies:** DNK-4 · **Size:** M

#### Acceptance criteria
- [x] An administrator invites a person by email; the invitation link is single-use, expires (config), and is stored hashed
- [x] Password reset sends a single-use, short-lived link; requesting it for an unknown email looks identical to a known one
- [x] Emails go through an outbox processed after commit; a failed send is retried with backoff
- [x] SMTP settings come from `DONKA_SMTP_URL` and `DONKA_SMTP_FROM`; local development uses Mailpit
- [x] Email content exists in English and French, chosen by the recipient's language

---

### DNK-6 — Studio web foundation

**Type:** feature · **Repos:** S · **Dependencies:** DNK-4 · **Size:** L

#### Why
Every screen after this one reuses its shell, language switch, theme, HTTP client and error
handling.

#### Acceptance criteria
- [ ] `apps/web` builds as a static export; `apps/app` serves it on the same origin, with unknown paths falling back to the app shell
- [ ] shadcn/ui on Radix with CSS-variable tokens, light and dark
- [ ] English and French with `i18n:check` in CI; language switch keeps the current page
- [ ] Sign-in and sign-out screens working against DNK-4, with the four data states
- [ ] Typed client generated from `/api/v1/openapi.json`; services return `ActionResult`
- [ ] Checked at 390 px and desktop; screenshots in the PR

---

### DNK-7 — Projects and who can work on them

**Type:** feature · **Repos:** S · **Dependencies:** DNK-6 · **Size:** M

#### Acceptance criteria
- [ ] Create, list, rename and archive projects; project keys are unique and URL-safe
- [ ] Members with roles owner / editor / viewer; only owners manage members
- [ ] One `ProjectAccess` extractor enforces membership on every project route; a non-member gets `404`
- [ ] A viewer calling any change endpoint gets `403`
- [ ] Tests prove a member of project A cannot read or change project B
- [ ] Screens: project list (empty state explains how to create one), project settings, members

---

### DNK-8 — Edit a decision and simulate it

**Type:** feature · **Repos:** S · **Dependencies:** DNK-7 · **Size:** L

#### Acceptance criteria
- [ ] Decisions (JDM graphs) are stored per project under a key (`person-score`, `bureau/normalize`)
- [ ] The editor page loads jdm-editor client-only, themed with Studio's tokens
- [ ] Changes autosave as a draft; two people editing the same decision get a conflict (`409`) instead of silently overwriting
- [ ] Simulate evaluates the draft with every sibling decision of the project, returns result + trace
- [ ] Viewers can open and simulate but not save

---

### DNK-9 — Version history and diff

**Type:** feature · **Repos:** S · **Dependencies:** DNK-8 · **Size:** M

#### Acceptance criteria
- [ ] "Save version" creates an immutable version with author, time and message
- [ ] History lists versions newest first, paginated
- [ ] Comparing two versions shows jdm-editor's diff view
- [ ] Restoring an old version creates a new version (history is never rewritten)

---

### DNK-10 — Test scenarios that run on every version

**Type:** feature · **Repos:** S · **Dependencies:** DNK-9 · **Size:** M

#### Why
An approver who only sees a diff is rubber-stamping. Tests make the four-eyes rule mean something.

#### Acceptance criteria
- [ ] A scenario = decision key + input + expected output (exact or partial match)
- [ ] All scenarios of a project run when a version is saved; results stored with the version
- [ ] A failing scenario shows expected vs actual per field
- [ ] Scenarios can be created from a simulator run in one click

---

### DNK-11 — Audit log

**Type:** feature · **Repos:** S · **Dependencies:** DNK-7 · **Size:** S

#### Acceptance criteria
- [ ] Every state change (sign-in, member change, save, release, deploy, approval, rollback) writes exactly one audit event in the same transaction
- [ ] Audit events cannot be updated or deleted, by the API or the application database role
- [ ] Owners can filter the project's audit log by person, action and date; export as CSV

---

### DNK-12 — Donka Runtime on engine 2.0.1, under its own name

**Type:** chore · **Repos:** R · **Dependencies:** DNK-1 · **Size:** M

#### Acceptance criteria
- [ ] zen-engine and zen-expression pinned to `=2.0.1`; Studio's drift check passes against the runtime
- [ ] Binary, image and service names are `donka-runtime`; upstream API paths unchanged
- [ ] Upstream release automation that would publish under GoRules names is removed
- [ ] CI runs fmt, clippy and all tests including the container-based ones

---

### DNK-13 — Runtime tokens stored hashed, one per environment

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-12 · **Size:** S

#### Acceptance criteria
- [ ] `.config/project.json` carries token hashes, never plain tokens; the artifact format is versioned and documented
- [ ] A token issued for staging is refused by the production runtime
- [ ] Artifacts in the old format keep working (additive change)

---

### DNK-14 — Releases and environments

**Type:** feature · **Repos:** S · **Dependencies:** DNK-10, DNK-11, DNK-13 · **Size:** L

#### Acceptance criteria
- [ ] Each project has exactly two environments: staging and production
- [ ] A release freezes one version of every decision in the project, with a semantic version and notes
- [ ] Deploying to staging writes the artifact to `staging/<project-key>` in the bucket after commit (outbox); a storage failure is retried and visible
- [ ] Studio issues, lists and revokes Runtime tokens per environment; a token is shown once
- [ ] An end-to-end test: release → deploy to staging → a Runtime reads it and evaluates

---

### DNK-15 — Production needs a second person's approval

**Type:** feature · **Repos:** S · **Dependencies:** DNK-14 · **Size:** M

#### Acceptance criteria
- [ ] Requesting production creates a pending approval and emails the project's approvers
- [ ] The release author cannot approve (`422 SELF_APPROVAL`)
- [ ] Approving publishes to `production/<project-key>`; rejecting records a reason
- [ ] Two approvers acting at the same time produce one outcome (`409` for the second)
- [ ] The approval screen shows diff, test results and release notes

---

### DNK-16 — Roll back production

**Type:** feature · **Repos:** S · **Dependencies:** DNK-15 · **Size:** S

#### Acceptance criteria
- [ ] An approver redeploys any previously approved release to production in one action, with a required reason
- [ ] No new approval is needed; the audit log records who, when, from which release to which
- [ ] A release never approved for production cannot be used for rollback

---

### DNK-17 — Connectors for bureau, KYC and AML calls

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-12 · **Size:** L

#### Decision
Connector handler implemented once in Rust (custom-node adapter), in an MIT crate inside
donka-runtime; Studio depends on it for simulation (mock mode).

#### Acceptance criteria
- [ ] Graph nodes store connector type, settings and a secret **name**; secret values come from the Runtime's environment or vault
- [ ] Timeout, bounded retries and a circuit breaker are enforced by the handler
- [ ] Studio simulation uses mock responses defined in the project; no real call leaves Studio
- [ ] A reference "bureau score" connector works end-to-end
- [ ] No secret value appears in artifacts, logs, traces or the decision log

---

### DNK-18 — Decision log: every production decision, searchable and replayable

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-14 · **Size:** L

#### Acceptance criteria
- [ ] The Runtime sends each evaluation (input, output, release, trace, time) to Studio asynchronously, batched, retried, without slowing the response
- [ ] Studio stores records encrypted at rest; retention from configuration; purge is audited
- [ ] Search by reference, decision, outcome and date; paginated
- [ ] Replay re-evaluates a record against its release and shows whether the result is identical
- [ ] Viewing a record is audited (it contains personal data)

---

### DNK-19 — Explain a decision in plain language

**Type:** feature · **Repos:** S · **Dependencies:** DNK-18 · **Size:** M

#### Acceptance criteria
- [ ] Off by default; enabling requires the customer's endpoint URL and key in configuration
- [ ] Fields listed in the project's redaction settings are removed before sending
- [ ] The explanation is in the viewer's language; it is read-only and cannot change any decision
- [ ] With the feature off, no explain control is rendered and the endpoint returns `404`

---

### DNK-20 — Pull releases from CI (Studio side)

**Type:** feature · **Repos:** S · **Dependencies:** DNK-14 · **Size:** M

#### Acceptance criteria
- [ ] `POST /api/v1/rules-sync` resolves `main`, `commit:<id>`, `release:<version>`, `env:<key>` targets and returns the artifact location and sha256
- [ ] Access with project tokens (read scope); tokens outside the project get `404`
- [ ] Unchanged targets answer `no_change` when the caller sends its current id

---

### DNK-21 — Donka CLI

**Type:** chore · **Repos:** C · **Dependencies:** DNK-20 · **Size:** M

#### Acceptance criteria
- [ ] Command `donka`, package `@donka/cli`, variables `DONKA_URL`, `DONKA_TOKEN`, `DONKA_PROJECT`, `DONKA_TARGET`
- [ ] `donka pull` works against DNK-20 for every target, with the same exit codes
- [ ] GitHub, GitLab and Azure templates renamed and tested
- [ ] Upstream publishing under GoRules names removed

---

### DNK-22 — Rate limits on the Runtime

**Type:** feature · **Repos:** R · **Dependencies:** DNK-12 · **Size:** S

#### Acceptance criteria
- [ ] Per-token request limit from configuration; exceeding it returns `429` with `Retry-After`
- [ ] Limits do not apply to health endpoints

---

### DNK-23 — Install Donka with one command

**Type:** feature · **Repos:** S · **Dependencies:** DNK-15, DNK-18 · **Size:** M

#### Acceptance criteria
- [ ] `docker compose --profile full up` starts Studio, Runtime, PostgreSQL and MinIO with working defaults
- [ ] Installation and operations guide in English and French (backup, upgrade, rotation of secrets)
- [ ] Post-deploy smoke script: health, ready, simulate, evaluate on the Runtime

---

### DNK-24 — Credit starter pack

**Type:** feature · **Repos:** S · **Dependencies:** DNK-14, DNK-17 · **Size:** M

#### Why
Customers buy time to a live scorecard, not an empty canvas.

#### Acceptance criteria
- [ ] Importable project "Retail credit": person scorecard with reason codes and bands
- [ ] Importable project "SME treasury evaluation"
- [ ] Each ships with test scenarios that pass and a short guide (EN/FR)

#### Decision
[INTERACTIVE STEP] Which bureaus and which currency/market conventions to model first.
- Option A: one country's credit bureau and currency, chosen with the pilot customer
- Option B: generic, bureau-agnostic scores only
Recommendation: A, once the pilot customer is known. Implementation of market-specific parts
stops here until decided.

---

### DNK-25 — Learn from the upstream codebases before inventing

**Type:** docs · **Repos:** S · **Dependencies:** DNK-3 · **Size:** S

#### Why
zen, agent-public and the CLI already solved many problems Donka meets. Copying good practice
(and knowing what not to copy) is cheaper than rediscovering it.

#### Acceptance criteria
- [ ] `docs/engineering/references.md` lists adopted, planned and rejected practices, each with its source file
- [ ] The engineering guides and `AGENTS.md` point to it
- [ ] Every planned practice has a story (DNK-26 to DNK-30)

---

### DNK-26 — Report which version is running

**Type:** feature · **Repos:** S · **Dependencies:** DNK-3 · **Size:** S

#### Acceptance criteria
- [x] `GET /api/v1/version` returns the version from the `SERVICE_VERSION` build argument (`unknown` in local builds)
- [x] The same version appears in the OpenAPI document and the startup log line

---

### DNK-27 — Optional OpenTelemetry

**Type:** feature · **Repos:** S · **Dependencies:** DNK-26 · **Size:** M

#### Acceptance criteria
- [ ] Traces and request metrics exported over OTLP when `DONKA_OTEL_ENABLED=true`; nothing exported by default
- [ ] The request id is attached to every span
- [ ] No personal data or secrets in span attributes

---

### DNK-28 — Fast, reproducible container builds

**Type:** build · **Repos:** S · **Dependencies:** DNK-3 · **Size:** S

#### Acceptance criteria
- [ ] The Dockerfile builds dependencies in a layer that only `Cargo.toml`/`Cargo.lock` changes invalidate
- [ ] The image runs as a non-root user and passes `SERVICE_VERSION` through
- [ ] CI builds the image on every pull request (no push yet)

---

### DNK-29 — Storybook for Studio components

**Type:** chore · **Repos:** S · **Dependencies:** DNK-6 · **Size:** S

#### Acceptance criteria
- [ ] Every shared UI component has a story showing its four states (loading, empty, error, success) where they apply
- [ ] Stories render in light and dark, English and French
- [ ] CI builds Storybook so a broken story fails the pull request

---

### DNK-30 — Changelog and versions from commits

**Type:** chore · **Repos:** S, R, C · **Dependencies:** DNK-1 · **Size:** S

#### Decision
[INTERACTIVE STEP] How release PRs are produced.
- Option A: release-please opens the `develop` → `main` release PR with the changelog and version bump generated from Conventional Commits
- Option B: release PRs written by hand, as in CONTRIBUTING.md today
Recommendation: A. It already runs in zen, cli and agent, and our commits follow the format it reads.

#### Acceptance criteria
- [ ] Release PRs carry a generated `CHANGELOG.md` entry and a semantic version bump
- [ ] Nothing is published to a package registry under GoRules names

---

### DNK-31 — Local stack starts, and engine drift fails CI

**Type:** build · **Repos:** S · **Dependencies:** DNK-12 · **Size:** S

#### Why
`minio/minio` and `minio/mc` were removed from Docker Hub, so `docker compose up` failed. The
engine drift check skipped silently in CI because the runtime was never checked out.

#### Acceptance criteria
- [ ] Every image in `docker-compose.yml` exists and is pinned to a release (no `latest`)
- [ ] CI checks out donka-runtime (`develop`) and fails when its zen-engine differs from Studio's
- [ ] The drift check fails instead of skipping when it cannot find the runtime

