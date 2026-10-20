# Foundation acceptance evidence

Audit date: 2026-10-20. Baseline: Studio `0cd78b8`, Runtime and CLI on their existing `develop`
checkouts. This audit reconciles DNK-1/2/3/25/31; it does not declare the pilot ready.
Source paths below are relative to the named repository. Validation is recorded separately
from code inspection so an unexecuted path is not presented as a live test.

## DNK-1 — shared playbook (four criteria)

| Criterion | Evidence |
|---|---|
| Playbook files without unresolved project placeholders | All three repos contain `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md` and `.github/pull_request_template.md`; project names and commands are filled. PR/story template prompts intentionally remain reusable author instructions. |
| Studio adaptations and decision links | `docs/engineering/backend.md` and `frontend.md`; ADR-001 separate Runtime, ADR-002 static same-origin web, ADR-003 engine boundary, ADR-004 registry UI/editor, ADR-005 TanStack Form, ADR-006 fork policy, ADR-007 own session pool. The decision index links each adaptation. |
| Forks keep upstream structure with workflow rules | Runtime/CLI `AGENTS.md`, `DONKA.md`, `CONTRIBUTING.md`; Studio ADR-006. Both retain their GoRules upstream remote and MIT license. |
| Manual repository settings listed | All three `CONTRIBUTING.md` files list default branch, merge modes and branch rules. This criterion is documentation of settings, not proof that every hosting plan enforces them. |

## DNK-2 — API conventions (five criteria)

| Criterion | Source and verification |
|---|---|
| Configurable route prefix | `apps/app/src/lib.rs` nests the router once; `config.rs` validates the base. `tests/api.rs::changing_the_base_path_moves_every_route` checks health, authentication and simulation under the alternate prefix and rejects the old one. |
| Simulation error meaning preserved | API tests `unknown_decision_is_404_decision_not_found`, `invalid_model_is_422_invalid_decision`, `engine_rejection_is_422_evaluation_failed_with_details`. |
| Safe unexpected failure and matching request id | `error.rs::IntoResponse` maps internal failures to `500 INTERNAL_ERROR` without the reason; `tests/api.rs::unexpected_failure_is_500_without_internal_detail` and `tests/support::assert_error` check safe content and identical body/header ids. Code inspection of `request_id.rs` verifies that the handler and its error event run inside the span carrying that same id; this is not a captured-log assertion. |
| Health independent of database | API test `health_does_not_need_the_database` uses a disconnected lazy pool. |
| Invalid configuration names its variable and stops startup | `config.rs` tests missing database URL, malformed base path and invalid values. `main.rs::exit_with` exits 2. Binary probes verify missing `DATABASE_URL` and invalid `DONKA_API_BASE_PATH` exit 2 with the variable name. |

## DNK-3 — persistence (five criteria)

| Criterion | Source and verification |
|---|---|
| Required database and configurable startup migration | `config.rs::{database_url_is_required,database_settings_are_validated}`, `main.rs` eager connect and conditional `donka_db::migrate`; `crates/db/tests/append_only.rs::migrations_apply_twice_without_error`. The migrate=false startup branch is inspected; configuration false is tested. |
| Readiness reflects database availability | API tests `ready_is_503_when_the_database_is_unreachable` and `ready_is_200_when_the_database_answers`; db tests cover both outcomes too. |
| Real PostgreSQL 16 integration tests in CI | `.github/workflows/ci.yml` supplies `postgres:16-alpine` and runs `cargo test --workspace --locked`; sqlx makes a database per test. |
| Reusable append-only guard | `migrations/20260928000001_append_only_guard.sql`; db tests permit inserts and reject updates, deletes and truncate on a fixture table. |
| Shared persistence crate and modules created with their data | `crates/db/src/lib.rs` owns pool/migrations/readiness; business crates have their own persistence; `Cargo.toml` lists the used modules. |

## DNK-25 — references (three criteria)

| Criterion | Evidence |
|---|---|
| Adopted/planned/rejected practices cite source files | `docs/engineering/references.md` now names concrete paths, moves delivered telemetry/build/release practices to adopted and identifies the next planned browser practice. |
| Guides and agent instructions point to references | `AGENTS.md` section 1, `docs/engineering/backend.md` and `frontend.md` introductions. |
| Planned practices have stories | DNK-26 to DNK-30 retain their backlog entries; DNK-48 tracks live feature browser tests. Every planned row names its story. |

## DNK-31 — stack and engine drift (three criteria)

| Criterion | Evidence |
|---|---|
| Published dependency images, no latest tag | `docker-compose.yml`: PostgreSQL `16-alpine`, MinIO `RELEASE.2026-08-04T00-00-00Z`, mc `RELEASE.2026-09-16T00-00-00Z`, Mailpit `v1.31.3`. All four exist in the local Docker image store with registry digests. Studio/Runtime images are built locally with `DONKA_VERSION`; they are not claimed as published dependencies. CI install builds and starts the full stack. |
| CI compares Runtime develop with Studio | `.github/workflows/ci.yml` checks out `youmssi/donka-runtime` at `develop`; `scripts/check-engine-version.sh` compares exact engine versions. Both currently pin 2.0.1. |
| Missing Runtime fails, mismatch fails | Run the existing script with the real Runtime, a missing directory and a temporary manifest pinning 9.9.9. Matching pins succeed; the latter probes exit nonzero with the expected error. |

## Verification and remaining readiness work

Local verification passed: Rust format, strict Clippy, all 256 workspace tests, executable build,
startup probes and engine-drift positive/negative probes. No product code changes are part of
this audit. PR CI still runs the complete five-job workflow. Web application source is unchanged
from DNK-29; its baseline has 197 tests, 950 catalog keys and 360 component-preview checks.

The broader audit found two requirements described in `principles.md` and `backend.md` that
are not enforced in current CI: the business-logic coverage floor and dependency-security
checks. DNK-49 must deliver these before declaring readiness. Do not describe them as passing
or weaken the documented requirements to match the workflow.

Full live feature journeys are also not yet committed as a repeatable application suite;
Storybook covers component previews. DNK-48 establishes the feature suite, with subsequent
feature-group stories extending it. Backup/restore and upgrade rehearsal is DNK-50.
See the [delivery plan](../delivery-plan.md) for phase exit conditions.
