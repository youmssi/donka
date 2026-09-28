# ADR-003: One zen-engine version, used only behind `DecisionRuntime`

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-1

## Context

The forks carried four engine versions (editor backend 0.53, jdm-editor WASM expression 0.55,
agent 1.0.0-beta.12, zen 2.0.1). If the simulator and production evaluate with different
versions, an approved rule can behave differently in production. Upgrading the engine should
also not ripple through route handlers.

## Decision

- Pin `zen-engine = "=2.0.1"` in Studio and in Donka Runtime; `scripts/check-engine-version.sh`
  fails CI on drift.
- Only `crates/engine` imports `zen_engine`; the rest of Studio uses the `DecisionRuntime` trait
  and evaluates whole project bundles so sub-decisions resolve as in a release.

## Consequences

- Engine upgrades are one pull request per repo, Runtime first.
- The expression grammar used for **in-editor linting** comes from `@gorules/jdm-editor`'s bundled
  WASM and may lag; it only affects hints, never results. Bump it when upstream publishes.
- Checked on 2026-09-28: donka-runtime compiles on 2.0.1 without code changes; its unit and
  non-container tests pass.
