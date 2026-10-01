# Architecture

## Two planes

**Control plane: Donka Studio** (this repo). Analysts, approvers, support and compliance use it.
It stores everything in Postgres and publishes immutable release artifacts.

**Data plane: Donka Runtime** (`youmssi/donka-runtime`). Customer systems call it. It reads
release artifacts from storage, hot-reloads them, and evaluates. If Studio is down, decisions
keep flowing. A bank can run the Runtime inside its own network zone.

The only coupling between the planes is the **release artifact** and the **decision log feed**.

## Release artifact

One zip per project and environment, the format the Runtime already understands:

```
<project-key>                  (object name, no extension)
├── .config/project.json       release id + version, environment, hashed access tokens
├── person-score.json          JDM graph
├── sme-treasury.json
└── bureau/normalize.json      sub-decisions keep their folder path
```

The format of `.config/project.json` and the token rules: [`artifact-format.md`](artifact-format.md).

Studio writes `staging/<project>` and `production/<project>` in the bucket. The Runtime polls
every 5 s and reloads on change. Releases are immutable. Rollback means redeploying an older
artifact.

## Components

| Component | Tech | Responsibility |
|---|---|---|
| `apps/web` | Next.js (static export), shadcn/ui + Radix + Tailwind, `@gorules/jdm-editor` (client-only) | All screens. Talks only to `apps/app`. |
| `apps/app` | Rust, Axum, sqlx, Postgres | Auth, projects, versions, tests, releases, approvals, audit, decision log, AI explain, rules-sync API |
| `crates/engine` | Rust, zen-engine 2.0.1 | `DecisionRuntime` trait + `ZenRuntime`. The only code that imports `zen_engine`. |
| Donka Runtime | Rust (agent fork) | Evaluate endpoints, OpenAPI per project, token checks, connector handler, decision-log emitter |
| Postgres | 16+ | Users, projects, versions, releases, approvals, append-only audit, decision log |
| MinIO / S3 | | Release artifacts |

## Request flows

1. **Author and test.** web → `POST /api/v1/simulate` with the whole draft project → `ZenRuntime`
   evaluates with trace → web shows the result per node. Connector nodes run in mock mode.
2. **Release.** Save creates an immutable version, and tests run. A release freezes one version of
   every decision in the project. Deploying to staging writes the artifact. Production needs one
   approval from someone other than the author, then the artifact is written.
3. **Evaluate.** Customer backend → Runtime `POST /api/rules/{project}/evaluate/{key}` with a
   bearer token → result + reason codes + release id. The Runtime ships the decision record to
   Studio asynchronously.
4. **Explain / replay.** Studio loads a logged decision, re-evaluates it against the same release
   (identical result), and can ask the customer's LLM for a plain-language explanation.

## Connectors (bureau, KYC, AML)

Implemented once, in Rust, as a zen-engine custom-node adapter (`DecisionEngine::with_adapter`),
shared by the Studio simulator and the Runtime. Graphs store the connector configuration and the
**name** of a secret. Values come from the Runtime's environment or vault. Timeouts, retries and a
circuit breaker are enforced by the handler, not by rule authors.

## Security baseline

- Session cookies (argon2 password hashes) for Studio users. Owner / editor / viewer per project.
- Runtime access tokens are hashed, scoped to one environment, and rotatable.
- Audit and decision-log tables are append-only at the database level.
- The browser never holds a Runtime token. Fieldkit calls the Runtime through the customer's backend.
