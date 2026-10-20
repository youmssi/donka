# Live feature tests

Playwright Test opens the production Studio UI and checks real service results. The initial
suite covers authentication plus project creation, input-field persistence, version/release
creation, token issuance, staging publication and a real Runtime evaluation.
The reset journey deliberately holds JavaScript requests to prove static form controls stay
disabled until hydration, then exercises the real email and password change. This guards the
lost-input race described in [Playwright's hydration guidance](https://playwright.dev/docs/navigations#hydration).
The [coverage map](#feature-coverage) keeps remaining workflows explicit.

## Environment

Use an isolated synthetic-data installation with Studio, PostgreSQL 16, MinIO/S3, Runtime and
Mailpit. The existing `docker compose --profile full up -d --build` starts this stack on a
fresh test machine; follow the [installation guide](../install.md) to set the initial admin
password. Do not share its database with customer data or use a real outbound mail service.

In that test installation's `.env`, set `DONKA_SIGN_IN_RATE_LIMIT=100` and
`DONKA_PASSWORD_RESET_RATE_LIMIT=100`: the locale/theme/browser matrix deliberately repeats
authentication. Production defaults are unchanged; default rate-limit behavior retains its
own integration tests and is part of the DNK-51 browser follow-up.

The runner requires these Node-only environment variables (also in `.env.example`):

| Variable | Test target |
|---|---|
| `DONKA_E2E_BASE_URL` | Studio origin, e.g. `http://localhost:8080` |
| `DONKA_E2E_RUNTIME_URL` | Runtime origin, e.g. `http://localhost:8090` |
| `DONKA_E2E_MAILPIT_URL` | Local email API, e.g. `http://localhost:8025` |
| `DONKA_E2E_ADMIN_EMAIL` | Administrator of that isolated installation |
| `DONKA_E2E_ADMIN_PASSWORD` | Its test password, supplied privately |

Origins must be HTTP(S), with no path or embedded credentials. Do not commit an environment
file or print these values. In Bash, `read -r -s DONKA_E2E_ADMIN_PASSWORD` followed by `export
DONKA_E2E_ADMIN_PASSWORD` avoids putting the password in command history. Use `$env:NAME` for
process variables in PowerShell. Optional `DONKA_TEST_BROWSER_CHANNEL=msedge` selects an
installed Edge for Chromium projects; Firefox/WebKit still use their own engines.

## Run

From the repository root with Node 22 and the repository's pnpm version:

```bash
pnpm --dir apps/web install --frozen-lockfile
pnpm --dir apps/web exec playwright install --with-deps chromium
pnpm --dir apps/web test:e2e
```

The default command selects Chromium: three tests across EN/FR, light/dark and desktop/390 px
(24 cases). To run one project while investigating a failure:

```bash
pnpm --dir apps/web exec playwright test --project=chromium-en-light-1440
```

Before a release, install all engines and run the same suite in all three (72 cases):

```bash
pnpm --dir apps/web exec playwright install --with-deps chromium firefox webkit
pnpm --dir apps/web test:e2e:release
```

CI reuses the full-stack install job after its smoke tests. Ordinary PRs run Chromium; PRs
targeting `main` and pushes to `main` install and run Chromium, Firefox and WebKit. Every case
uses its own browser context and unique project/recipient data. Tests are serial, with no
retries; waiting assertions observe real readiness, mail delivery and artifact reloads.

Global setup checks Studio and Mailpit, then signs in the test admin once. The release journey
checks Runtime readiness before changing project data. A private temporary auth-state file
outside report directories reaches workers; teardown
signs that session out and removes the file. Individual auth tests sign out their users, and
the release test revokes its Runtime token in cleanup. Separate service request contexts keep
Studio session cookies away from Runtime and Mailpit.

`playwright-report/` and `test-results/` are ignored by Git, formatting, lint and Docker build
context. Failure traces/screenshots are retained; CI keeps failure artifacts for three days.
They can contain synthetic test data and disposable test authentication: keep local artifacts
private and never attach raw traces or auth state to a public PR. PR screenshots must exclude
full tokens and passwords. Persistent admin auth state is never included in these artifacts.

## Feature coverage

A passing initial suite does not certify every feature. UI actions under test use the browser;
API calls create clearly identified fixtures or independently check persisted/service results.
The known graph comes from `crates/engine/tests/fixtures/table.json`; graph-node authoring and
the simulation panel are not claimed by that seeded fixture.

| Area | Initial evidence | Remaining live browser coverage |
|---|---|---|
| Identity | Invitation email, setup/mismatch, independent password reveal, keyboard reveal, required sign-in fields, sign-in/out, delayed-hydration guard, reset email and single-use reset | DNK-51: expired/invalid links, wrong-password/lockout/rate limits, account administration and permission boundaries |
| Projects | Create through UI, returned project data and independent fixture key | DNK-51: members/roles, cross-project denial, edit/archive/concurrency |
| Rules and contracts | Known graph fixture; edit a field through UI and verify persistence after reload | DNK-52: graph authoring, simulation panel, all field types/limits, raw schema, breaking contract changes |
| Versions/scenarios | Save the current draft as a version through UI | DNK-52: diff/restore, scenario edit/run/failure paths |
| Releases/Runtime | UI release, shown-once token, staging publication, actual Runtime result and unknown-token rejection | DNK-53: second-person approval, rejection, rollback, revocation behavior, scopes/rate limits and connector failures |
| Records/explanations/audit | Existing Rust and API e2e checks remain separate evidence | DNK-54: record ingestion/search/replay, redaction/explain states, audit UI, retention/key rotation |
| Packs/onboarding/CLI | Existing component/unit/API checks remain separate evidence | DNK-55: import/duplicate/export, checklist/tours and real CLI sync/contracts |
| Foundation/operations | [Foundation audit](foundation-evidence.md) and existing install/API e2e jobs | DNK-49 coverage/security gates; DNK-50 restore/upgrade rehearsal |

Close these gaps story by story before the Phase 1 exit gate. Add new feature journeys in the
same implementation story; keep error assertions explicit rather than hiding a failure with
a retry, timeout increase or a skipped test.
