# ADR-006: How donka-runtime and donka-cli follow the playbook

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-1

## Context

donka-runtime and donka-cli are forks of MIT projects that keep receiving upstream fixes.
Restructuring their code or API paths to match Studio's conventions (`/api/v1`, module crates)
would make every upstream merge a conflict and break compatibility with existing artifacts and
CI templates.

## Decision

- Apply the workflow rules fully: Conventional Commits with `Refs: DNK-<n>`, `develop` and story
  branches, squash merges, PR template, no AI authorship trace, checks before push.
- Keep upstream's code structure, API paths and tooling. Donka changes stay small and behind
  extension points.
- Keep the `upstream` remote; merge upstream into a `dnk-<n>-upstream-sync` story branch.
- Keep upstream's `LICENSE` and copyright unchanged.

## Consequences

- The forks look different inside from Studio; that is intended.
- Upstream release automation that publishes under GoRules names must be disabled in the
  rebrand stories (DNK-12, DNK-21).
