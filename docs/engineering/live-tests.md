# Live feature tests

Playwright Test opens the production Studio UI and checks real service results. The initial
suite covers authentication, People, project membership/settings, permission boundaries,
input-field persistence, version/release creation, token issuance, staging publication and a
real Runtime evaluation. A separate service restart exercises actual authentication throttling.
The reset journey deliberately holds JavaScript requests to prove static form controls stay
disabled until hydration, then exercises the real email and password change. This guards the
lost-input race described in [Playwright's hydration guidance](https://playwright.dev/docs/navigations#hydration).
The [coverage map](#feature-coverage) keeps remaining workflows explicit.

The release journey cold-loads each screen in a fresh tab and reopens the editor to verify
persisted fields. This avoids [WebKit's document-replacement cancellation bug](https://github.com/microsoft/playwright/issues/43120),
which can report departing-document requests as access-control page errors. Every tab remains
under the same page-error assertion; no errors are filtered. Each new tab is brought to the
foreground before loading, and the previous tab closes; background tabs pause publication
polling. Feature-group stories below extend client-side navigation coverage.

## Environment

Use an isolated synthetic-data installation with Studio, PostgreSQL 16, MinIO/S3, Runtime and
Mailpit. The existing `docker compose --profile full up -d --build` starts this stack on a
fresh test machine; follow the [installation guide](../install.md) to set the initial admin
password. Do not share its database with customer data or use a real outbound mail service.

In that test installation's `.env`, set `DONKA_SIGN_IN_RATE_LIMIT=100` and
`DONKA_PASSWORD_RESET_RATE_LIMIT=100`: the locale/theme/browser matrix deliberately repeats
authentication. Production defaults are unchanged; default rate-limit behavior retains its
own PostgreSQL integration tests. The separate security suite below runs with small limits so
it can check actual throttling without exhausting the main journey suite's shared address.

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

The default command selects Chromium: eight tests across EN/FR, light/dark and desktop/390 px
(64 cases). To run one project while investigating a failure:

```bash
pnpm --dir apps/web exec playwright test --project=chromium-en-light-1440
```

Before a release, install all engines and run the same suite in all three (192 cases):

```bash
pnpm --dir apps/web exec playwright install --with-deps chromium firefox webkit
pnpm --dir apps/web test:e2e:release
```

### Authentication rate limits

After the main suite finishes, restart **only the disposable test installation's Studio**
with `DONKA_SIGN_IN_RATE_LIMIT=2` and `DONKA_PASSWORD_RESET_RATE_LIMIT=1`. With Compose, edit
those values in that installation's `.env`, run `docker compose --profile full up -d --wait app`
and wait for `/api/v1/ready`. Restarting clears the process-local address budgets. Do not run
other sign-in/reset clients concurrently against this process.

Set the Node runner's `DONKA_E2E_SIGN_IN_RATE_LIMIT=2` and
`DONKA_E2E_PASSWORD_RESET_RATE_LIMIT=1` to match, retain the five environment variables above,
then run:

```bash
pnpm --dir apps/web test:e2e:security          # eight Chromium cases
pnpm --dir apps/web test:e2e:security:release  # 24 cases across all engines
```

Setup first proves a fresh process allows exactly the configured burst and rejects its next
request on each endpoint. This suite does not sign in an administrator or use its saved state.
Each case opens the real sign-in/reset form, consumes the configured budget through real HTTP
calls, then checks the
browser submission returns `429`, matching `Retry-After`/`retryAfterSeconds`, no session cookie,
translated retry guidance and preserved input. No response is mocked. The deliberately small
budgets give a 30/60-second refill interval; no test waits for a wall-clock window to expire.
Restore the main suite's 100/100 values and restart Studio before running its journeys again.
CI runs the main suite first and this separate restart/suite last. Security reports use their
own ignored `playwright-report-security/` directory.

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
| Identity | Invitation email, setup/mismatch, independent password reveal, keyboard reveal, required fields, sign-in/out, hydration guard, reset/single use; missing/forged/replaced links; unknown/wrong/locked account generic failure; live configured rate limits and retry guidance | Clock-dependent invitation/reset/session expiry and lockout recovery use PostgreSQL integration tests, not a timed browser wait |
| People | UI invitation with language/admin flag, pending status, resend replacing the old link, activated status without resend; non-admin UI/API denial | Account activation in the People journey is an API fixture; password acceptance has its separate UI journey |
| Projects | UI create; invite-and-add unknown person; viewer/editor/owner role changes; last-owner refusal; removal cancellation/confirmation; rename/description persisted on a fresh page; archive cancellation/confirmation/read-only/server rejection/restore | Concurrent removals/demotions use PostgreSQL integration tests; broad feature-specific permission coverage continues in DNK-52–55 |
| Permission boundaries | Viewer read-only UI; viewer/editor project/member mutations denied; viewer decision creation denied/editor allowed; other project UI unavailable/read-write 404; non-admin accounts/invitations/project creation denied; removed membership immediately loses access | Instance-admin non-membership and the full project route matrix use PostgreSQL integration tests; member fixtures sign in through the API |
| Rules and contracts | Known graph fixture; edit a field through UI and verify persistence in a fresh page load | DNK-52: graph authoring, simulation panel, all field types/limits, raw schema, breaking contract changes |
| Versions/scenarios | Save the current draft as a version through UI | DNK-52: diff/restore, scenario edit/run/failure paths |
| Releases/Runtime | UI release, shown-once token, staging publication, actual Runtime result and unknown-token rejection | DNK-53: second-person approval, rejection, rollback, revocation behavior, scopes/rate limits and connector failures |
| Records/explanations/audit | Existing Rust and API e2e checks remain separate evidence | DNK-54: record ingestion/search/replay, redaction/explain states, audit UI, retention/key rotation |
| Packs/onboarding/CLI | Existing component/unit/API checks remain separate evidence | DNK-55: import/duplicate/export, checklist/tours and real CLI sync/contracts |
| Foundation/operations | [Foundation audit](foundation-evidence.md) and existing install/API e2e jobs | DNK-49 coverage/security gates; DNK-50 restore/upgrade rehearsal |

Close these gaps story by story before the Phase 1 exit gate. Add new feature journeys in the
same implementation story; keep error assertions explicit rather than hiding a failure with
a retry, timeout increase or a skipped test.

## Identity/project integration evidence

Run `cargo test -p donka-app --test auth --test invitations --test projects --locked` with
`DATABASE_URL` pointing at the isolated PostgreSQL test service. Do not overlap sqlx suites
on the same database service. These tests use the real handlers, migrations and database;
their injected clock makes expiry/recovery deterministic. They do not replace deployed
browser evidence or claim SMTP is real in the integration fixture.

| Boundary | Exact integration journeys |
|---|---|
| Link expiry | `a_setup_link_expires`, `an_invitation_link_expires`, `a_reset_link_is_short_lived` |
| Session and lockout | `five_failures_lock_the_account_for_fifteen_minutes`, `concurrent_failures_are_all_counted`, `a_reset_link_sets_a_new_password_and_ends_sessions`; the auth suite also checks idle expiry and CSRF |
| Client throttling | `sign_in_is_limited_per_client_address`, `password_reset_is_limited_per_client_address`, `forwarded_for_counts_only_through_a_trusted_proxy`; deployed UI suite separately checks configured 2/1 limits |
| Membership | `a_member_of_one_project_cannot_read_or_change_another`, `viewers_and_editors_cannot_change_the_project_or_its_members`, `administrators_are_not_implicit_members` |
| Owner concurrency | `two_owners_demoting_each_other_at_once_leave_one_owner`, `concurrent_owner_removals_and_demotions_keep_one_owner` (remove/remove and remove/demote; exactly one succeeds and one owner remains) |

DNK-51 closes this feature group's evidence gap after these checks pass. It does not close
DNK-52–55, the DNK-49 security/coverage gates or the DNK-50 operational rehearsal.

DNK-51 validation (2026-10-20): 64 main Chromium cases and eight rate-limit cases pass in all
eight UI modes, with no failures, retries or skips in the final runs. An additional eight-case
People run verifies changing the invitation's email language and independently checking the
stored value. The full Rust suite passes 258 tests, including the exact journeys above. Web
format/lint/types, 197 tests across 42 files, 950 matching catalog keys and the production export
pass. The [journey sources](../../apps/web/e2e/) and
[rate-limit sources](../../apps/web/security-probes/) are committed with the story. The full
three-engine matrix remains a release gate; these results certify Chromium for this PR.
