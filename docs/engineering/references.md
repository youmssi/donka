# Practices learned from the upstream codebases

Donka is built next to mature open-source code: `gorules/zen` (engine), `gorules/agent-public`
(runtime), `gorules/cli`, `gorules/jdm-editor` and `gorules/editor`. Before inventing a
convention, check how they solved it. This page records what we adopted, what we deliberately
did not, and why. Update it when a new practice is taken from them.

## Adopted

| Practice | Seen in | Where it lives in Donka |
|---|---|---|
| `forbid(unsafe_code)`, `deny(clippy::unwrap_used)` | zen (`core/*/src/lib.rs`) | `[workspace.lints]` in `Cargo.toml`; tests allowed via `clippy.toml` |
| `--locked` builds, least-privilege `permissions`, `concurrency` cancel-in-progress, `RUST_BACKTRACE=1` in CI | zen (`.github/workflows/rust.yaml`) | `.github/workflows/ci.yml` |
| Release binaries with LTO, one codegen unit, stripped symbols | zen, agent (`Cargo.toml` profiles) | `[profile.release]` |
| Engine evaluations on a pinned worker pool (`LocalPoolHandle`) because QuickJS futures are not `Send` | editor (`backend/src/main.rs`), agent (`src/app.rs`, `src/routes/engine.rs`) | `crates/engine` (`ZenRuntime`) |
| OpenAPI generated from handlers with `utoipa` + `utoipa-axum` (same versions as the runtime) | agent (`src/app.rs`) | `apps/app/src/lib.rs` |
| Evaluate whole projects through an in-memory loader so sub-decisions resolve | zen (`core/engine/src/loader/memory.rs`) | `Bundle` in `crates/engine` |
| Integration tests against real services, never substitutes | agent (`tests/it/support/minio.rs`, `Cargo.toml`) | `sqlx::test` + PostgreSQL 16 service in CI |
| Retry only transport errors, 408, 429 and 5xx; "a 4xx is an answer, not a blip"; bounded attempts with backoff | cli (`src/api/client.ts`) | Rule for every provider client (`backend.md` §6) |
| Never print a token; mask it in CI templates | cli (`src/api/client.ts`) | Security rules (`backend.md` §8) |
| Atomic file writes (temp file + rename) so a reader never sees a partial file | cli (`src/api/extract.ts`) | Rule for artifact publishing (DNK-14) |
| Lenient parsing of the artifact config: a missing field degrades that field, never the whole config | agent (`src/data/release_data.rs`) | Rule for the artifact contract (DNK-13, DNK-14) |
| Exit codes as a documented contract | cli (`README.md`, `src/api/client.ts`) | Studio startup: exit 2 for operator-fixable failures |
| Test and local services pinned to maintained images: `pgsty/minio` replaces `minio/minio` (removed from Docker Hub) | donka-runtime (`tests/it/support/minio.rs`, DNK-12) | `docker-compose.yml` |
| `GET /version` from a `SERVICE_VERSION` build argument, also in the OpenAPI `info.version`. Adapted: compiled in with `option_env!` (the agent reads it at runtime, so the environment could misreport it) and answered as JSON `{ version }` so fields can be added | agent (`src/routes/infra.rs`, `Dockerfile`) | `apps/app/src/lib.rs` (`VERSION`), `apps/app/Dockerfile` |
| TypeScript `strict` + `noUncheckedIndexedAccess`; Prettier `printWidth: 120`, `singleQuote` | cli (`tsconfig.json`, `.prettierrc`) | `frontend.md` §10 (applies from DNK-6) |
| Typed CSF stories for shared UI, previewed in both locales/themes and checked in a browser | jdm-editor (`packages/jdm-editor/.storybook/main.ts`, `packages/jdm-editor/src/components/decision-graph/dg.stories.tsx`) | `apps/web/.storybook`, `apps/web/src/stories`, [component previews](storybook.md) (DNK-29) |
| Optional OpenTelemetry traces and metrics, off by default | agent (`src/telemetry.rs`) | `apps/app/src/telemetry.rs`, `apps/app/tests/telemetry.rs` (DNK-27) |
| Cached dependency layers in the container build | agent (`Dockerfile`) | `apps/app/Dockerfile`, CI image/install jobs (DNK-28) |
| release-please versions and changelogs from Conventional Commits | zen (`.github/workflows/release-please.yaml`), cli (`.github/workflows/release.yml`), agent (`release-please-config.json`) | `.github/workflows/release.yml`, `release-please-config.json` (DNK-30) |

## Planned (stories in the backlog)

| Practice | Source | Story |
|---|---|---|
| Committed live business journeys with isolated data, user-facing locators and waiting assertions | [Playwright best practices](https://playwright.dev/docs/best-practices) | DNK-48; extend per feature group |

DNK-26 to DNK-30 were originally planned here; their delivered practices now appear above.
See [foundation evidence](foundation-evidence.md) for the audit and the remaining CI gates.

## Deliberately not adopted

| Practice | Seen in | Why not |
|---|---|---|
| `.expect()` / `panic!` on invalid configuration at startup | agent (`src/main.rs`, `src/app.rs`) | Operators get a stack trace instead of the variable name. Studio prints one line and exits 2. |
| Listen address chosen by build type (`127.0.0.1:3000` in debug, `0.0.0.0:8080` in release) | agent (`src/main.rs`), editor (`backend/src/main.rs`) | Hardcoded configuration. Studio reads `DONKA_LISTEN`. |
| A single `CORS_PERMISSIVE` switch | editor (`backend/src/main.rs`) | Studio is same-origin; allowed origins, when needed, come from configuration (`backend.md` §8). |
| `continue-on-error: true` on quality checks | cli (`.github/workflows/validate.yml`) | Failures were silently ignored, including jobs that never started. Checks must block merges. |
| `auth` disabled for the engine's HTTP module (`ZEN_CONFIG.http_auth = false`) | editor (`backend/src/main.rs`) | Outbound calls from rules go through Donka connectors with their own credentials (DNK-17). |
