# Live feature tests

Playwright Test opens the production Studio UI and checks real service results. The committed
suite covers authentication, People, project membership/settings, permission boundaries,
graph authoring/simulation, input contracts, versions/scenarios, release creation, token
issuance, production approval/rejection/rollback, token isolation/revocation, connector
failure policies, staging publication and real Runtime evaluations. A separate service restart
exercises actual authentication throttling.
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
Mailpit. The feature override adds a production Runtime, a staging Runtime limited to three
requests per minute per token, and a controlled bureau. Start the disposable stack with:

```bash
docker compose -f docker-compose.yml -f scripts/feature-services.compose.yml --profile full up -d --build
```

Keep the same two `-f` arguments for subsequent Compose commands in this section. The override
contains a public synthetic bureau secret and must only be used for tests. The Runtime uses
`DONKA_SECRET_E2E_BUREAU_KEY`; graphs store only `E2E_BUREAU_KEY`. The bureau reports received
synthetic bodies and an authentication boolean, never the secret or request headers.
The existing `docker compose --profile full up -d --build` starts the basic stack on a
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
| `DONKA_E2E_PRODUCTION_RUNTIME_URL` | Runtime serving `production/`, e.g. `http://localhost:8091` |
| `DONKA_E2E_LIMITED_RUNTIME_URL` | Staging Runtime with requests=3/window=60000 ms, e.g. `http://localhost:8092` |
| `DONKA_E2E_BUREAU_URL` | Controlled service reachable by Playwright, e.g. `http://localhost:8099` |
| `DONKA_E2E_CONNECTOR_URL` | Same service reachable by Runtime, e.g. `http://feature-bureau:8099` on Compose |
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

The default command selects Chromium: thirteen tests across EN/FR, light/dark and desktop/390 px
(104 cases). To run one project while investigating a failure:

```bash
pnpm --dir apps/web exec playwright test --project=chromium-en-light-1440
```

Before a release, install all engines and run the same suite in all three (312 cases):

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
`DONKA_E2E_PASSWORD_RESET_RATE_LIMIT=1` to match, retain the environment variables above,
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
The release and contract journeys explicitly seed `crates/engine/tests/fixtures/table.json`.
The editor journey instead creates an empty decision through the UI, drags its three nodes,
connects them, types its expression and runs the simulator. These are separate evidence.

At narrow widths the simulator keeps usable pane widths inside a labelled horizontal scroll
area. The editor journey focuses the actual Monaco input, types with the keyboard and scrolls
the output into view. It does not inject editor state or replace service responses.

| Area | Current evidence | Remaining live browser coverage |
|---|---|---|
| Identity | Invitation email, setup/mismatch, independent password reveal, keyboard reveal, required fields, sign-in/out, hydration guard, reset/single use; missing/forged/replaced links; unknown/wrong/locked account generic failure; live configured rate limits and retry guidance | Clock-dependent invitation/reset/session expiry and lockout recovery use PostgreSQL integration tests, not a timed browser wait |
| People | UI invitation with language/admin flag, pending status, resend replacing the old link, activated status without resend; non-admin UI/API denial | Account activation in the People journey is an API fixture; password acceptance has its separate UI journey |
| Projects | UI create; invite-and-add unknown person; viewer/editor/owner role changes; last-owner refusal; removal cancellation/confirmation; rename/description persisted on a fresh page; archive cancellation/confirmation/read-only/server rejection/restore | Concurrent removals/demotions use PostgreSQL integration tests; broad feature-specific permission coverage continues in DNK-52–55 |
| Permission boundaries | Viewer read-only UI; viewer/editor project/member mutations denied; viewer decision creation denied/editor allowed; other project UI unavailable/read-write 404; non-admin accounts/invitations/project creation denied; removed membership immediately loses access | Instance-admin non-membership and the full project route matrix use PostgreSQL integration tests; member fixtures sign in through the API |
| Rules and contracts | UI node creation/connections/expression, fresh-page graph persistence, real simulation and save-as-scenario; all four field types, range validation, enums, labels/PII, raw JSON validation and array/pattern schema persistence; real breaking/compatible review | Sibling resolution, simultaneous draft conflicts, viewer permissions and invalid saved schemas retain PostgreSQL integration tests; approval request/publication for the contract review are API fixtures |
| Versions/scenarios | UI save/unchanged guard, passing/failing scenario execution, field-level mismatch, draft/version and version/version diff, restore cancellation/confirmation/new version, scenario edit/JSON validation/exact mode/delete; immutable captured results after deletion | History pagination, missing sibling versions, project-wide scenario snapshots and viewer/isolation boundaries retain PostgreSQL integration tests |
| Releases/Runtime | UI release, shown-once token, staging publication, actual Runtime result and contract violations; separate owner signs in and rejects/approves production, author cannot decide, required reasons, approved rollback cancellation/confirmation restores the earlier Runtime result; live token project/environment isolation, UI revocation, independent rate budgets and health exemption | Graphs, versions, staging publication, token issuance and second-owner account activation in DNK-53 are API fixtures; concurrent decisions, stale staging requests and permission matrices retain PostgreSQL integration coverage |
| Connectors | UI settings/secret validation and fresh-page persistence; actual Studio simulation returns its mock without contacting the bureau; Runtime sends mapped nested JSON with the named secret, retries a transient 503, applies bounded timeout/failure/non-JSON/missing-secret fallback, stops when configured to fail and opens its breaker after five failures without making a sixth call | Additional failure-policy graphs are API fixtures; connector maximum caps, template errors and breaker cooldown retain connector integration tests |
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

## Editor/contract integration evidence

The browser journeys above complement the real PostgreSQL handler tests in
`apps/app/tests/{decisions,versions,scenarios,releases}.rs`:

| Boundary | Integration journeys |
|---|---|
| Shared drafts | `two_people_saving_the_same_draft_get_a_conflict_not_an_overwrite`, `two_saves_at_the_same_moment_give_one_save_and_one_conflict`; hook tests separately exercise both conflict choices |
| Simulation and permissions | `simulate_evaluates_the_draft_with_every_sibling_decision`, `viewers_open_and_simulate_but_cannot_change`, `decisions_stay_inside_their_project` |
| Version safety | `versions_cannot_be_changed_or_deleted`, `history_lists_versions_newest_first_a_page_at_a_time`, `viewers_read_the_history_but_cannot_save_or_restore`, `a_deleted_decision_keeps_its_history_and_frees_its_key` |
| Contract safety | `saving_a_version_reports_fields_the_contract_and_the_rules_disagree_on`, `a_version_with_a_broken_contract_is_refused`; release tests check frozen artifact schemas and approval comparisons |
| Scenario snapshots | `editors_write_scenarios_with_a_unique_name_per_decision`, `viewers_read_scenarios_and_other_projects_see_nothing`, `every_version_runs_the_project_scenarios_and_keeps_the_results` (including missing saved sibling versions) |

These are integration evidence, not additional deployed-browser claims. The Runtime journey
uses the real published artifact and token; it checks number bounds, integer type, enums,
booleans and array patterns, while the core release journey checks required nested fields.

DNK-52 validation (2026-10-20): 80 main Chromium cases and eight live rate-limit cases pass
across all eight UI modes. An eight-case editor follow-up verifies the same workflows with
the page returned to the top for the PR captures. The reports contain no failures, retries or
skips. Web formatting/lint/types, 197 tests across 42 files, 950 matching catalog keys, the
production export and Storybook build pass. Rust formatting, strict Clippy, all 258 tests and
Studio/Runtime engine-version agreement (2.0.1) pass. All 360 Storybook browser render checks,
including the component interactions, pass. The three-engine matrix remains a release
gate; this story's deployed browser evidence is Chromium.

## Approval/Runtime/connector integration boundaries

The deployed DNK-53 journeys use different browser sessions for the author and second owner.
The second owner signs in through the UI. Invitation/password activation, project graphs,
versions, release snapshots and staging deployments are explicitly seeded through the API.
Production requests, rejection, approval, rollback and token revocation use the UI; independent
API and Runtime calls verify their effects. Rollback changes the real evaluated output from
36 back to 24, and verifies the release id. Synthetic data and isolated tokens keep the
three-request budget independent for every UI mode.

The connector journey configures one node in the real editor, saves it, cold-loads it and
simulates it. The controlled service has zero calls until Runtime evaluation. Each failure
fixture has a unique URL, so unrelated failures cannot trip its breaker. The service's call
count, rendered nested body and authentication boolean are checked independently of the
Runtime trace. Delayed answers intentionally arrive after the node's 100 ms timeout; this
service delay is test input, not a browser synchronization sleep.

The following boundaries retain separate integration evidence:

| Boundary | Exact integration journeys |
|---|---|
| Atomic approval | Studio `two_owners_deciding_at_once_produce_one_outcome`, `the_releases_author_cannot_approve_it`, `a_project_needs_another_owner_to_approve` |
| Publication snapshot | Studio `environment_reads_stay_consistent_during_publication` reads while the real publisher commits; live/latest/token reads use a read-only repeatable-read transaction so the UI cannot stop polling on a mixed snapshot |
| Rejection/withdrawal/rollback | Studio `rejecting_records_a_reason_and_withdrawing_is_for_the_asker`, `only_a_release_approved_for_production_can_come_back`, `a_rollback_keeps_waiting_requests_and_survives_token_changes` |
| Publication and roles | Studio `a_storage_failure_is_retried_and_visible`, `a_newer_deployment_supersedes_one_not_yet_published`, `viewers_read_releases_but_cannot_release_or_deploy` |
| Connector bounds and errors | Runtime connector crate, `live::retries_are_capped_by_the_runtime`, `live::a_refusal_or_a_non_json_answer_is_not_retried`, `live::an_unreachable_service_falls_back_too`, `live::errors_never_carry_the_secret` |
| Breaker recovery | Runtime connector crate, `live::the_circuit_opens_after_repeated_failures` checks opening, suppression and the probe after cooldown |

Run Studio's full `cargo test --workspace --locked` suite against PostgreSQL without another
sqlx suite on that service. Run the Runtime connector crate with
`cargo test -p donka-connectors --features live --locked` for its real local HTTP fixtures.
These are distinct from the deployed Playwright evidence and do not replace the release
matrix or the remaining DNK-54/55 journeys.
