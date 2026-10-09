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
- [x] `apps/web` builds as a static export; `apps/app` serves it on the same origin, with unknown paths falling back to the app shell
- [x] shadcn/ui on Radix with CSS-variable tokens, light and dark
- [x] English and French with `i18n:check` in CI; language switch keeps the current page
- [x] Sign-in and sign-out screens working against DNK-4, with the four data states
- [x] Typed client generated from `/api/v1/openapi.json`; services return `ActionResult`
- [x] Checked at 390 px and desktop; screenshots in the PR

---

### DNK-7 — Projects and who can work on them

**Type:** feature · **Repos:** S · **Dependencies:** DNK-6 · **Size:** M

#### Decisions (taken in the story, easy to change)
- Only instance administrators create projects; the creator becomes the first owner.
- Administrators are not implicit members of every project (least privilege); an owner adds them.
- Keys are immutable (artifacts are stored under them); archiving is reversible by an owner.

#### Acceptance criteria
- [x] Create, list, rename and archive projects; project keys are unique and URL-safe
- [x] Members with roles owner / editor / viewer; only owners manage members
- [x] One `ProjectAccess` extractor enforces membership on every project route; a non-member gets `404`
- [x] A viewer calling any change endpoint gets `403`
- [x] Tests prove a member of project A cannot read or change project B
- [x] Screens: project list (empty state explains how to create one), project settings, members

---

### DNK-8 — Edit a decision and simulate it

**Type:** feature · **Repos:** S · **Dependencies:** DNK-7 · **Size:** L

#### Decisions (taken in the story)
- The draft is a working copy: autosaves are not audit events; creating and deleting a decision
  are. Immutable versions, and their audit trail, come with DNK-9.
- On a conflict the editor stops autosaving and asks: load their version, or keep mine (which
  replaces theirs, knowingly). Nothing is overwritten silently.
- Projects open on their decisions.

#### Acceptance criteria
- [x] Decisions (JDM graphs) are stored per project under a key (`person-score`, `bureau/normalize`)
- [x] The editor page loads jdm-editor client-only, themed with Studio's tokens
- [x] Changes autosave as a draft; two people editing the same decision get a conflict (`409`) instead of silently overwriting
- [x] Simulate evaluates the draft with every sibling decision of the project, returns result + trace
- [x] Viewers can open and simulate but not save

---

### DNK-9 — Version history and diff

**Type:** feature · **Repos:** S · **Dependencies:** DNK-8 · **Size:** M

#### Decisions (taken in the story)
- Versions are numbered per decision (v1, v2…) and hold the whole graph. The database refuses
  any change to a version, as it does for the audit log.
- A version needs a message (up to 500 characters). Saving when the draft equals the latest
  version is refused (`VERSION_UNCHANGED`): no empty versions. "Equal" compares the graphs, not
  revisions, so a save that changes nothing leaves the draft unchanged.
- A version is taken of the draft the editor shows: pending autosaves are sent first, and the
  request names the draft revision, so a concurrent edit is a conflict, not a surprise.
- Restoring version N puts its graph in the draft and adds a new version ("Restored version N")
  in one transaction, with the same revision check. Draft changes not saved as a version are
  replaced; the confirmation says so.
- Deleting a decision is now a soft delete: its versions stay (a release will point at them);
  its key is free for a new decision.
- The editor and the decisions list show the latest version and whether the draft changed
  since. Saving and restoring a version are audit events.
- Compare: any two versions, or a version and the draft, in jdm-editor's diff view (added,
  removed, modified nodes). Viewers can compare but not save or restore.

#### Acceptance criteria
- [x] "Save version" creates an immutable version with author, time and message
- [x] History lists versions newest first, paginated
- [x] Comparing two versions shows jdm-editor's diff view
- [x] Restoring an old version creates a new version (history is never rewritten)

---

### DNK-10 — Test scenarios that run on every version

**Type:** feature · **Repos:** S · **Dependencies:** DNK-9 · **Size:** M

#### Why
An approver who only sees a diff is rubber-stamping. Tests make the four-eyes rule mean something.

#### Decisions (taken in the story)
- A scenario targets one decision of the project: a name (unique per decision, any case), an
  input object, an expected output object, and a match. **Partial**: every expected field has
  its value, other output fields are free. **Exact**: the output is exactly the expected
  document. Objects compare field by field, arrays and values as a whole, numbers by value.
- Saving or restoring any version runs **every** scenario of the project, because decisions
  call each other. A version is tested as a release would hold it: that decision at the new
  version, every other decision at its latest version. A scenario that needs a decision without
  a saved version is reported as "could not run", naming that decision.
- Results never block saving; they are kept with the version, never change, and copy the
  scenario as it ran, so editing or deleting a scenario later does not rewrite what was tested.
  Approvals (DNK-14) will read them.
- Mismatches (path, expected, actual) are computed by the server, so API clients get the same
  per-field explanation as the web app.
- Editors and owners write scenarios; viewers read them and the results. Creating, changing and
  deleting a scenario are audit events.
- "Save as scenario" in the simulator keeps the last run's input and output (partial match by
  default) for the person to name and trim.

#### Acceptance criteria
- [x] A scenario = decision key + input + expected output (exact or partial match)
- [x] All scenarios of a project run when a version is saved; results stored with the version
- [x] A failing scenario shows expected vs actual per field
- [x] Scenarios can be created from a simulator run in one click

---

### DNK-11 — Audit log

**Type:** feature · **Repos:** S · **Dependencies:** DNK-7 · **Size:** S

#### Acceptance criteria
- [x] Every state change (sign-in, member change, save, release, deploy, approval, rollback) writes exactly one audit event in the same transaction — done for every change that exists today (account and project changes); save, release, deploy, approval and rollback record theirs in their own stories (backend guide §5 and the endpoint checklist)
- [x] Audit events cannot be updated or deleted, by the API or the application database role
- [x] Owners can filter the project's audit log by person, action and date; export as CSV

---

### DNK-12 — Donka Runtime on engine 2.0.1, under its own name

**Type:** chore · **Repos:** R · **Dependencies:** DNK-1 · **Size:** M

#### Acceptance criteria
- [x] zen-engine and zen-expression pinned to `=2.0.1`; Studio's drift check passes against the runtime
- [x] Binary, image and service names are `donka-runtime`; upstream API paths unchanged
- [x] Upstream release automation that would publish under GoRules names is removed
- [x] CI runs fmt, clippy and all tests including the container-based ones

---

### DNK-13 — Runtime tokens stored hashed, one per environment

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-12 · **Size:** S

#### Decisions (taken in the story)
- Format version 2 adds `accessTokenHashes`: per token its id, environment, algorithm
  (`sha256`) and lowercase hex digest. Tokens are long random secrets (at least 32 random
  bytes), so an unsalted SHA-256 is enough; the Runtime compares in constant time.
- An artifact deployed to an environment accepts only entries issued for that environment.
- A malformed entry is skipped, never the whole file: a file that fails to parse turns token
  checks off in the Runtime, so a bad entry must not be able to do that.
- The Runtime ships the change; Studio documents the format (`docs/artifact-format.md`) and
  starts writing it when it issues tokens and deploys releases (DNK-14).

#### Acceptance criteria
- [x] `.config/project.json` carries token hashes, never plain tokens; the artifact format is versioned and documented
- [x] A token issued for staging is refused by the production runtime
- [x] Artifacts in the old format keep working (additive change)

---

### DNK-14 — Releases and environments

**Type:** feature · **Repos:** S · **Dependencies:** DNK-10, DNK-11, DNK-13 · **Size:** L

#### Decisions (product owner)
- A decision with no saved version blocks the release; the dialog names it with a link to fix it.
- Editors and owners create releases and deploy to staging; owners manage Runtime tokens.
  Production goes through DNK-15's approval.
- The person picks a major, minor or patch bump; the first release of a project is 1.0.0.
- Failing tests warn but do not block; each frozen version keeps its test results.

#### Technical choices
- A release copies the frozen content (`release_decisions`, append-only), so a later version or a
  deleted decision never changes what was released.
- Each deployment row is its own outbox entry: published after commit by a worker
  (`FOR UPDATE SKIP LOCKED`, backoff from 30 s to 1 h, `DONKA_PUBLISH_MAX_ATTEMPTS`), then marked
  published, retrying or failed with the last error; a failed one is retried by hand.
- A newer deployment of the same environment supersedes the unpublished ones, so the bucket
  always ends with the latest request.
- Issuing or revoking a token republishes the live release with the new hash list.
- Storage is `object_store` behind `ArtifactStore`: `s3://` (AWS, MinIO, Ceph) or `file:///`.

#### Acceptance criteria
- [x] Each project has exactly two environments: staging and production
- [x] A release freezes one version of every decision in the project, with a semantic version and notes
- [x] Deploying to staging writes the artifact to `staging/<project-key>` in the bucket after commit (outbox); a storage failure is retried and visible
- [x] Studio issues, lists and revokes Runtime tokens per environment; a token is shown once
- [x] An end-to-end test: release → deploy to staging → a Runtime reads it and evaluates (`scripts/e2e.sh`, CI job `e2e`)

---

### DNK-15 — Production needs a second person's approval

**Type:** feature · **Repos:** S · **Dependencies:** DNK-14 · **Size:** M

#### Decisions (product owner)
- Approvers are the project's owners.
- Neither the person who made the release nor the person who asked for production can approve
  or reject it (`422 SELF_APPROVAL`); a request with no other owner is refused (`422 NO_APPROVER`).
- Only the release live on staging can be asked for production (`422 NOT_ON_STAGING`).
- One request per project waits at a time (`409 APPROVAL_PENDING`); the person who asked can
  withdraw it.

#### Technical choices
- Approvals live in the release module (`crates/release/src/approval.rs`): approving queues the
  production deployment through the same outbox as staging.
- Deciding locks the pending request first (`SELECT … FOR UPDATE`), then queues the deployment
  and records the outcome; a second owner waits on the lock and gets `409 APPROVAL_DECIDED`.
- Approval emails have their own outbox (`approval_emails`), rendered in each owner's language
  when queued (they hold no secret), sent by the email worker with the same retry policy.
- The review compares the release with what production ran before the request's outcome, by
  decision id (a renamed decision is the same decision); each changed decision opens the graph
  diff of its two versions.

#### Acceptance criteria
- [x] Requesting production creates a pending approval and emails the project's approvers
- [x] The release author cannot approve (`422 SELF_APPROVAL`)
- [x] Approving publishes to `production/<project-key>`; rejecting records a reason
- [x] Two approvers acting at the same time produce one outcome (`409` for the second)
- [x] The approval screen shows diff, test results and release notes

---

### DNK-16 — Roll back production

**Type:** feature · **Repos:** S · **Dependencies:** DNK-15 · **Size:** S

#### Decisions (product owner)
- Any owner can roll back, including to a release they made or asked for: it was approved by
  someone else once, and rollback is an emergency action.
- Targets are the releases once approved for production, except the live one, newer ones
  included (so production can come back after a rollback).
- A request waiting for approval stays pending; its review compares with production as it is.

#### Technical choices
- A rollback is a production deployment with reason `rollback` and its `rollback_reason`, queued
  through the same outbox; token changes republish the release production runs, rolled back or not.
- `GET /projects/{id}/rollback-targets` lists the targets; `POST /projects/{id}/rollbacks` rolls
  back (`422 NEVER_APPROVED`, `422 ALREADY_LIVE`).

#### Acceptance criteria
- [x] An approver redeploys any previously approved release to production in one action, with a required reason
- [x] No new approval is needed; the audit log records who, when, from which release to which
- [x] A release never approved for production cannot be used for rollback

---

### DNK-17 — Connectors for bureau, KYC and AML calls

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-12 · **Size:** L

#### Decision
Connector handler implemented once in Rust (custom-node adapter), in an MIT crate inside
donka-runtime; Studio depends on it for simulation (mock mode).

#### Decisions (taken in the story)
- Connector types: HTTP JSON (POST) and a "bureau score" template of it.
- On failure after the retries, the author chooses per node: fail the evaluation, or continue
  with a fallback answer. Only service failures fall back; bad settings never do.
- Mock answers live on each connector node; simulation and test scenarios use them.
- Secret values come from the Runtime's environment (`DONKA_SECRET_<NAME>`); a vault is a later
  story. The decision log (DNK-18) receives the node trace, which never holds a secret.

#### Acceptance criteria
- [x] Graph nodes store connector type, settings and a secret **name**; secret values come from the Runtime's environment or vault
- [x] Timeout, bounded retries and a circuit breaker are enforced by the handler
- [x] Studio simulation uses mock responses defined in the project; no real call leaves Studio
- [x] A reference "bureau score" connector works end-to-end
- [x] No secret value appears in artifacts, logs, traces or the decision log

---

### DNK-18 — Decision log: every production decision, searchable and replayable

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-14 · **Size:** L

#### Decisions (taken in the story)
- **Outcome**: each project names one output field (a dotted path, e.g. `decision`); Studio reads
  it when a record arrives and stores it searchable. Failed evaluations have the outcome `error`.
- **Retention**: five years by default (`DONKA_DECISION_LOG_RETENTION_DAYS=1825`).
- **Access**: every project member searches the log and opens records; each opening and replay
  is audited. Owners set the outcome field.
- **Runtime credential**: decision-log tokens issued by administrators under **Runtimes**, one
  environment each, shown once and stored hashed; a staging Runtime cannot write production
  records, and Studio stores a record only for a release it published to that environment.
- **Reference**: callers send `X-Donka-Reference` to the Runtime; every logged answer returns
  `X-Decision-Id`. The trace is always recorded and returned only when asked for.
- **Encryption**: input, output, error and trace are encrypted with AES-256-GCM under
  `DONKA_DECISION_LOG_KEY` (required); search fields stay readable.
- **Replay** runs the record's release in Studio with connector nodes answering from the
  recorded trace (no outside call). Identical means both succeeded with the same output, or
  both failed.
- Feed format: `docs/decision-log-feed.md`.

#### Acceptance criteria
- [x] The Runtime sends each evaluation (input, output, release, trace, time) to Studio asynchronously, batched, retried, without slowing the response
- [x] Studio stores records encrypted at rest; retention from configuration; purge is audited
- [x] Search by reference, decision, outcome and date; paginated
- [x] Replay re-evaluates a record against its release and shows whether the result is identical
- [x] Viewing a record is audited (it contains personal data)

---

### DNK-19 — Explain a decision in plain language

**Type:** feature · **Repos:** S · **Dependencies:** DNK-18 · **Size:** M

#### Decisions (taken in the story)
- **Endpoint formats**: Anthropic's Messages API or OpenAI-compatible chat completions
  (`DONKA_EXPLAIN_API`), so a bank can use a hosted model or one it runs itself. URL, key, format
  and model are all configuration; nothing is assumed.
- **Redaction**: owners list dotted paths (`applicant.nationalId`) in the project's decision-log
  settings; they are removed from the input, the output and every trace node (and from a
  decision table's traced references) before sending. Values computed from a redacted field are
  not tracked.
- **Who**: any member who can open the record; each explanation is audited
  (`decision_record.explained`) before the record is sent, whatever the endpoint answers.
- **Nothing stored**: the explanation is shown, not kept; asking again asks the endpoint again.

#### Acceptance criteria
- [x] Off by default; enabling requires the customer's endpoint URL and key in configuration
- [x] Fields listed in the project's redaction settings are removed before sending
- [x] The explanation is in the viewer's language; it is read-only and cannot change any decision
- [x] With the feature off, no explain control is rendered and the endpoint returns `404`

---

### DNK-20 — Pull releases from CI (Studio side)

**Type:** feature · **Repos:** S · **Dependencies:** DNK-14 · **Size:** M

#### Decisions (taken in the story)
- **Targets**: Donka has releases, not branches or commits. `main` is the project's newest
  release; `commit:<id>` is a release by its id; `release:<version>` by its semantic version;
  `env:<key>` what is live there. `branch:` answers `UNSUPPORTED_TARGET`.
- **Tokens**: new read-only **CI tokens**, issued per project by owners in Settings, shown once,
  hashed, revocable, audited, with the time they were last used. Runtime tokens are not reused.
- **Other projects**: a sync entry for a project the token cannot reach answers `no_access`
  (the CLI's contract); downloading another project's artifact answers `404`.
- **Artifacts**: an environment's artifact is its Runtime's (tokens included); a release's has
  no environment and no token. Both are rebuilt reproducibly, so the SHA-256 holds.
- **no_change** compares the caller's id with the release (or, for `env:`, the deployment) it
  resolved to. Contract: `docs/rules-sync.md`.

#### Acceptance criteria
- [x] `POST /api/v1/rules-sync` resolves `main`, `commit:<id>`, `release:<version>`, `env:<key>` targets and returns the artifact location and sha256
- [x] Access with project tokens (read scope); tokens outside the project get `404`
- [x] Unchanged targets answer `no_change` when the caller sends its current id

---

### DNK-21 — Donka CLI

**Type:** chore · **Repos:** C · **Dependencies:** DNK-20 · **Size:** M

#### Decisions (taken in the story)
- **Shipping**: nothing goes to npm. Each GitHub release of donka-cli (tagged `vX.Y.Z`) carries
  `donka-cli-X.Y.Z.tgz`; the templates run it with `npx --package`, and a `cli-package` setting
  points them at a mirror inside the bank's network.
- **Webhook payload**: removed from the templates. Studio does not start pipelines, so each
  pipeline sets its project and target; a Studio-triggered pipeline would be a later story.
- **Tests**: donka-cli tests the built CLI and each template's script against a fake Studio that
  follows `docs/rules-sync.md`, and CI runs the GitHub action itself.

#### Acceptance criteria
- [x] Command `donka`, package `@donka/cli`, variables `DONKA_URL`, `DONKA_TOKEN`, `DONKA_PROJECT`, `DONKA_TARGET`
- [x] `donka pull` works against DNK-20 for every target, with the same exit codes
- [x] GitHub, GitLab and Azure templates renamed and tested
- [x] Upstream publishing under GoRules names removed

---

### DNK-22 — Rate limits on the Runtime

**Type:** feature · **Repos:** R · **Dependencies:** DNK-12 · **Size:** S

#### Acceptance criteria
- [x] Per-token request limit from configuration; exceeding it returns `429` with `Retry-After`
- [x] Limits do not apply to health endpoints

---

### DNK-23 — Install Donka with one command

**Type:** feature · **Repos:** S · **Dependencies:** DNK-15, DNK-18 · **Size:** M

#### Acceptance criteria
- [x] `docker compose --profile full up` starts Studio, Runtime, PostgreSQL and MinIO with working defaults
- [x] Installation and operations guide in English and French (backup, upgrade, rotation of secrets)
- [x] Post-deploy smoke script: health, ready, simulate, evaluate on the Runtime

---

### DNK-24 — Credit starter pack

**Type:** feature · **Repos:** S · **Dependencies:** DNK-14, DNK-17 · **Size:** M

#### Why
Customers buy time to a live scorecard, not an empty canvas.

#### Acceptance criteria
- [x] Importable project "Retail credit": person scorecard with reason codes and bands
- [x] Importable project "SME treasury evaluation"
- [x] Each ships with test scenarios that pass and a short guide (EN/FR)

#### Decision
Option A, decided by the product owner: the pilot customer's market, **Cameroon (CEMAC)**, with
amounts in **FCFA (XAF)** and reason codes in English and French. Bureau data is an input the
lender's system fills from the source it consults (a private bureau or the central bank's risk
register), so the packs do not depend on one bureau's API. Policy values are expert defaults,
marked for the pilot's risk team to confirm; they are not regulatory values.

Packs live in `packs/` (format 1: manifest, decisions, scenarios, guides) and are imported with
`scripts/import-pack.sh`; CI imports each one and checks its scenarios. Import from the web app is
DNK-43.

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
- [x] Traces and request metrics exported over OTLP when `DONKA_OTEL_ENABLED=true`; nothing exported by default
- [x] The request id is attached to every span
- [x] No personal data or secrets in span attributes

---

### DNK-28 — Fast, reproducible container builds

**Type:** build · **Repos:** S · **Dependencies:** DNK-3 · **Size:** S

#### Acceptance criteria
- [x] The Dockerfile builds dependencies in a layer that only `Cargo.toml`/`Cargo.lock` changes invalidate
- [x] The image runs as a non-root user and passes `SERVICE_VERSION` through
- [x] CI builds the image on every pull request (no push yet)

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
Option A, signed off 2026-09-28: release-please generates the changelog and version bump from
Conventional Commits (as in zen, cli and agent). It runs on `main`: a promotion PR brings
`develop` in, release-please keeps a release PR open, merging it tags the release and opens a
back-merge PR into `develop`. (Option B, hand-written release PRs, was declined.)

#### Acceptance criteria
- [x] Release PRs carry a generated `CHANGELOG.md` entry and a semantic version bump
- [x] Nothing is published to a package registry under GoRules names

---

### DNK-32 — Rate limits on sign-in and password reset

**Type:** feature · **Repos:** S · **Dependencies:** DNK-5 · **Size:** S

#### Why
Lockout protects one account, and one pending email per user protects one inbox. Nothing limits
how fast one client can try many accounts or ask for many reset emails (found in DNK-5).

#### Acceptance criteria
- [x] Per-client-address limits on `POST /auth/sign-in` and `POST /auth/password-reset` from
      configuration; exceeding one returns `429 RATE_LIMITED` with `Retry-After`
- [x] The client address honours a configured trusted proxy (`X-Forwarded-For` only from it)
- [x] The web app shows the translated message and when to retry

---

### DNK-33 — People: invite and manage Studio accounts

**Type:** feature · **Repos:** S · **Dependencies:** DNK-7 · **Size:** S

#### Why
Invitations exist in the API (DNK-5) but not in the web app, and owners can only add people who
already have an account (found in DNK-7).

#### Acceptance criteria
- [x] Administrators see Studio's accounts (email, administrator or not, invited or active) on a People page
- [x] Administrators invite a person (email, language, administrator or not) and resend a pending invitation
- [x] In a project's members screen, an administrator adding an unknown email is offered to invite them
- [x] English and French; checked at 390 px and desktop

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

---

### DNK-34 — Studio works like a web app: sidebar shell and one component system

**Type:** feature · **Repos:** S · **Dependencies:** DNK-11 · **Size:** M

#### Why
Studio grew screen by screen with hand-made lists, tabs, pagers and empty states. It should feel
like one web application: persistent navigation, breadcrumbs, a quick switcher, dense tables,
short dates and readable links, built from the shadcn components instead of our own.

#### Decisions (product owner, 2026-09-29)
- Collapsible **sidebar** navigation (icons when collapsed, a sheet on phones), breadcrumb in the
  page header, command palette (Ctrl/⌘ K).
- **Data table** (TanStack Table) for every list: projects, members, people, audit.
- Project links use the project **key** (`?p=credit-pme`), not its id.
- shadcn components are taken from the official registry (new-york-v4) and not rewritten;
  forms use TanStack Form + Zod through shadcn `Field`.

#### Acceptance criteria
- [x] Signed-in pages share one shell: sidebar (brand, Projects, People for administrators, the
      open project's sections, account menu with language and theme), breadcrumb, command palette
- [x] Every form uses `Field` with TanStack Form + Zod; submit buttons show a spinner; a
      success is confirmed by a toast
- [x] Projects, members, people and audit are data tables with loading skeleton, empty state
      (`Empty`) with its next action, error with retry, and pagination
- [x] Dates are short ("13 Nov", "13 Nov 2025" for another year, "14:05") with the full date and
      time on hover; recent audit events read "2 h ago"
- [x] Project pages open by key; `?id=` links keep working
- [x] Checked at desktop and 390 px, English and French, light and dark

---

### DNK-35 — Every screen built from the shadcn components

**Type:** refactor · **Repos:** S · **Dependencies:** DNK-15 · **Size:** S

#### Why
Screens added after DNK-34 (releases, environments, approvals) and a few older spots still use
hand-made lists, labels, status boxes and a pager. Studio should look and behave the same
everywhere, using the shadcn component made for each job and its documented usage.

#### Decisions
- Lists of things with actions or badges are `Item` / `ItemGroup`; form labels are `Field`.
- A status that needs attention (a deployment on its way or given up, a request waiting) is an
  `Alert`.
- Pagination is the shadcn `Pagination`; a value to copy is an `InputGroup` with its button.
- The two graph diff views (version history and approval review) share one panel.

#### Acceptance criteria
- [x] No hand-made list, label, pager or status box remains where a shadcn component fits
- [x] Behaviour, text and tests are unchanged except where a component improves accessibility
- [x] Screens checked on desktop and 390 px, English and French, light and dark

---

### DNK-36 — READMEs that present each repository

**Type:** docs · **Repos:** S, R, C · **Dependencies:** DNK-21 · **Size:** S

#### Why
The READMEs grew as reference manuals. Someone landing on a repository should see at a glance
what it is, what it does, how to try it and where to read more, the same way in all three.

#### Decisions
- One layout everywhere: banner, centred name and tagline, badges, links; then Introduction,
  Features, Quick start, Documentation, Contributing, License.
- Badges show facts only (CI, license, stack, how it ships). No contributor counts.
- Reference content moves to `docs/` pages linked from the README (Runtime: configuration,
  connectors, decision log, rules OpenAPI; CLI: pull, CI templates, MCP bridge); nothing is lost.
- One banner design (`.github/assets/banner.svg`) per repository, in Studio's colours.

#### Acceptance criteria
- [x] The three READMEs follow the same layout and link to each other
- [x] Every section removed from a README lives on in a `docs/` page
- [x] Version pins in the CLI docs are still updated by release-please

---

### DNK-37 — One input contract for the rules and the form

**Type:** feature · **Repos:** S, R · **Dependencies:** DNK-14, DNK-20 · **Size:** L

#### Why
The fields a credit decision needs are defined twice today: implicitly in the rules (what the
expressions read) and again by hand in the application form. When an analyst renames or adds a
field, nothing tells the form, and the mismatch is found in production. Analysts and form
builders need one definition, released with the rules, that both sides are checked against.

#### Decision
- **JSON Schema on the input node of the project's entry decision** is the contract.
  zen-engine 2.0.1 already validates requests against it, so the Runtime enforces the same
  definition the form is built from. The engine validates with **draft-07** (found while
  building it; the story first said 2020-12), so contracts are written as draft-07: the keywords
  a form needs (type, required, limits, allowed values, format) are the same in both.
- **Presentation hints** live beside it as `x-donka` annotations the engine ignores: label and
  help in English and French, widget, order, step, `pii: true`. The form may override them.
- **Released with the rules**: the schema is frozen in each release and travels in the artifact
  and over rules-sync, so `release 1.4.0` always means the same fields.
- Not an export/import file: an imported copy drifts silently; a pinned, checked contract does
  not.

#### Behaviour

| Where | Before | After |
|---|---|---|
| Studio, decision editor | Input fields are implicit | An **Input fields** table: name, type, required, limits, allowed values, format, labels (EN/FR), PII |
| Studio, saving a version | No field checks | Warns when a rule reads a field missing from the contract, or a required field no rule reads |
| Studio, approval screen | Rules diff only | Also lists contract changes, marked **breaking** (removed, renamed, now required, narrower) or **compatible** |
| Release artifact / rules-sync | Decisions only | Adds `input.schema.json` per entry decision (additive) |
| Runtime | Accepts any input | Answers `400` naming the field when a request breaks the contract (engine validation) |
| Decision log | Redaction listed by hand | Fields marked `pii` are redacted from explanations by default |

#### Acceptance criteria
- [x] Analysts edit the input fields of a decision in a table; the JSON Schema is generated, and
      editing the raw schema stays possible
- [x] The schema is saved with each version and frozen in each release
- [x] Saving warns about fields read by rules but not declared, and declared required fields no
      rule reads
- [x] The approval screen shows contract changes and flags breaking ones
- [x] Artifacts and rules-sync carry `input.schema.json` (artifact format: additive, documented)
- [x] The Runtime refuses a request that breaks the contract with `400` and the field's path
- [x] `pii` fields are redacted from explanations without listing them again
- [x] Labels and help in English and French

#### Out of scope
- Rendering forms (Fieldkit), and the CLI side (DNK-38)
- Output contracts (what a decision returns), a later story

---

### DNK-38 — Forms pull and check the input contract

**Type:** feature · **Repos:** C · **Dependencies:** DNK-37 · **Size:** M

#### Why
A form team needs to start from the contract and know, in CI, the day their form stops matching
the rules it feeds.

#### Behaviour

| Command | What it does |
|---|---|
| `donka form pull --project credit-pme --target env:production` | Downloads the input contract(s) of a target, like `donka pull` does artifacts |
| `donka form check --contract input.schema.json --form <form definition>` | Fails (exit `1`) when a field is missing, renamed, of another type, or required on one side only; lists every difference |

#### Acceptance criteria
- [x] `donka form pull` resolves every rules-sync target and verifies the checksum
- [x] `donka form check` reports each difference with its field path; exit codes follow the CLI's
      contract
- [x] The GitHub, GitLab and Azure templates gain a contract check step, tested like the pull
- [x] A form definition can be generated from the contract as a starting point

#### Out of scope
- Fieldkit's own renderer (Stage 2); the check reads a plain JSON Schema-based form definition
  so any form library can use it

---

### DNK-39 — One logo family for Donka and its products

**Type:** chore · **Repos:** S, R, C · **Dependencies:** DNK-36 · **Size:** S

#### Why
Studio shows a letter "D" as its logo and the READMEs carry the same placeholder. Donka needs a
real mark that also tells Studio, Runtime, CLI and Fieldkit apart.

#### Decision
Direction A of the proposal: the "Decision D", a D holding an input that runs into a decision
node. One rounded tile and stroke for the family; each product has its own glyph and colour
(Runtime: D with a forward chevron, teal; CLI: a prompt, slate; Fieldkit: a field and a ticked
checkbox, amber). The wordmark is lowercase **donka** in Geist Bold, outlined.

#### Acceptance criteria
- [x] Marks and light/dark lockups for Donka, Studio, Runtime, CLI and Fieldkit in `docs/brand/`,
      with usage rules
- [x] Studio's header, sidebar and browser tab show the mark, coloured by the theme
- [x] Studio's and the CLI's README banners show their product's mark
- [x] The Runtime's README banner shows its mark (after DNK-22, one story at a time per repo)

---

### DNK-44 — One form hook for Studio and Fieldkit

**Type:** refactor · **Repos:** S · **Dependencies:** DNK-34, DNK-37 · **Size:** M

#### Why
Every Studio form builds its own `useForm`, passes an untyped field (`AnyFieldApi`) to its
inputs and repeats the same submit button. TanStack Form's own guidance is to wrap it once in
an app form hook (`createFormHook`) with pre-bound field and form components. That keeps forms
short and typed end to end, and gives Fieldkit (Stage 2) the components it will pick per field
from an input contract (ADR-005: one form engine, shared field patterns).

#### Behaviour

| Where | After |
|---|---|
| `components/shared/form/` | `createFormHookContexts` + `createFormHook` export `useAppForm` and `withForm`; field components (text, number, select, checkbox, …) read their field with `useFieldContext<T>()`; `SubmitButton` reads the form with `useFormContext()` |
| Every form | Uses `useAppForm` and `form.AppField` / `form.AppForm`; no `form.Subscribe` copied for the submit button; large forms (input fields, connector node) split with `formOptions` + `withForm` |
| Failed submit | Focus moves to the first invalid field (`onSubmitInvalid`, `aria-invalid`) |
| Person using Studio | Nothing else changes: same fields, messages, timing of errors and layout |

#### Acceptance criteria
- [x] One app form hook in `components/shared/form/`; no form calls `useForm` directly, and no
      field component takes an `AnyFieldApi` prop
- [x] Every existing form moved to it with its behaviour unchanged; its tests still pass
- [x] A submit that fails validation focuses the first invalid field, tested
- [x] Validation timing kept as the frontend guide states (`onChange` + `onSubmit`, errors shown
      once the field is left or the form submitted)
- [x] The frontend guide and ADR-005 describe the hook and how to add a field component
- [x] `@tanstack/react-form` pinned to an exact patch version (its types change between patches)

#### Out of scope
- Fieldkit itself and rendering forms from a contract (Stage 2)
- New validation behaviour (`revalidateLogic`, async validators)

---

### DNK-40 — Rotate the decision-log key

**Type:** feature · **Repos:** S · **Dependencies:** DNK-18, DNK-23 · **Size:** M

#### Why
Every decision record is encrypted with `DONKA_DECISION_LOG_KEY`, and Studio reads records with
that key only. A key that may have leaked cannot be replaced today without losing every record
written before: the install guide tells operators not to change it.

#### Acceptance criteria
- [x] Studio takes a new key and keeps reading records sealed with the previous ones (each record
      names its key: `key_id`)
- [x] New records are sealed with the new key only
- [x] A command re-seals old records with the new key, resumable, audited; afterwards the old key
      can be removed
- [x] The install and operations guide (EN/FR) describes the rotation

---

### DNK-41 — First-run onboarding

**Type:** feature · **Repos:** S · **Dependencies:** DNK-24, DNK-43 · **Size:** M

#### Why
A new analyst lands on an empty project list. They should reach "my first decision answered by
the Runtime" in minutes, and find their way around the editor and the release screens without a
training session.

#### Decisions
- A **Get started** checklist on the projects page, driven by real state from the API (a project
  exists, a simulation ran, a version is saved, a release is live on staging, a Runtime token
  exists), not by "tour seen" flags. Each step links to where it is done; it disappears once done.
- Its first step imports a starter pack (DNK-43).
- Short guided tours (4 to 6 steps) with driver.js (MIT, no dependencies, works with the static
  export): the decision editor, releases and environments. Each runs once, can be replayed from a
  Help menu, and can always be skipped.
- Tours are translated (next-intl), follow the theme, work with the keyboard and respect reduced
  motion. "Seen" is kept per user on the server, so tours do not replay on another device.

#### Acceptance criteria
- [ ] The checklist shows each step's real state and links to it; it hides when complete
- [ ] Editor and releases tours, English and French, replayable from Help, skippable
- [ ] Tour state per user on the server (additive API)
- [ ] Checked at 390 px and desktop, light and dark

---

### DNK-42 — Wording that fits any industry

**Type:** chore · **Repos:** S · **Dependencies:** DNK-19 · **Size:** S

#### Why
Donka decides anything that takes JSON in and gives JSON out: insurance, mobile money,
e-commerce, public sector. A few texts assume a bank and a loan applicant, which tells a
non-bank prospect the product is not for them.

#### Acceptance criteria
- [ ] The explanation prompt speaks of "the organisation's business rules" and "the person or case
      decided", not of a bank and an applicant
- [ ] Hints and placeholders use neutral examples (`customer.id`, "Order discounts")
- [ ] English and French; product docs keep credit as the first example, not the only one

---

### DNK-43 — Packs: import, duplicate, export

**Type:** feature · **Repos:** S · **Dependencies:** DNK-24, DNK-37 · **Size:** M

#### Why
Starter packs import with a script today (DNK-24). Analysts should import one from the web app,
copy any project to start another product, and turn their own project into a pack another team or
installation can import.

#### Decisions
- The pack format is DNK-24's (`packs/README.md`), extended with the input contract (DNK-37).
- What a copy carries: decisions (latest draft or a chosen version), test scenarios, input
  contract, decision-log settings. What it never carries: releases, deployments, tokens, decision
  records (personal data) and members (the person copying becomes the only owner).
- A copy has no link back: a newer pack never overwrites it. Its audit log starts with where it
  came from ("created from pack Retail credit 1.2", "duplicated from project X").

#### Acceptance criteria
- [x] Import a pack from a catalogue in the web app, under a key and name of one's choice
- [x] Duplicate a project; export a project as a pack file and import it in another installation
- [x] Packs "KYC risk rating" and "Mobile money tiered limits" (CEMAC), with scenarios and guides
- [x] Audited; English and French
