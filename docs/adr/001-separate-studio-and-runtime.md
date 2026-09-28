# ADR-001: Separate Studio (authoring) from Runtime (serving)

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-1

## Context

Customer systems call Donka for credit decisions at any time; analysts use Studio during working
hours. The upstream `gorules/agent-public` (MIT) already serves decisions from release artifacts in
object storage, with typed evaluate endpoints, OpenAPI per project, hot reload and access tokens.
Banks often require the decision service inside their own network zone.

## Options considered

1. **Studio also serves production decisions** (`/api/evaluate` in `apps/app`). One container.
   Cons: rebuilds what the agent already does; a Studio outage or deploy stops decisions; authors
   and customer traffic share one process and one attack surface.
2. **Separate Runtime (fork of agent-public) + Studio**, coupled only through immutable release
   artifacts and an asynchronous decision-log feed.
3. **Both** (Runtime embedded in Studio for small installs). Doubles the paths to test.

## Decision

Option 2. Studio publishes project-level release artifacts (zip + `.config/project.json`) to
MinIO/S3 per environment; Donka Runtime polls, hot-reloads and evaluates.

## Consequences

- Decisions keep flowing when Studio is down or being upgraded; the Runtime scales on its own.
- The artifact format is a contract (additive changes only; Runtime ships first when it changes).
- One more container and MinIO in every installation.
- Rollback is redeploying an older, already approved artifact.
