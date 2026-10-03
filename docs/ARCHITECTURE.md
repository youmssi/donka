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
├── person-score               JDM graph, named by its decision key
├── sme-treasury
└── bureau/normalize           sub-decisions keep their folder path
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
   bearer token (and optionally `X-Donka-Reference`) → result + release id + `X-Decision-Id`.
   The Runtime ships the decision record to Studio asynchronously, in batches.
4. **Explain / replay.** Studio loads a logged decision, re-evaluates it against the same release
   (identical result), and can ask the customer's LLM for a plain-language explanation.

## Connectors (bureau, KYC, AML)

Implemented once, in Rust, as a zen-engine custom-node adapter (`DecisionEngine::with_adapter`):
the MIT crate `donka-connectors` in `donka-runtime/crates/connectors`. Studio depends on it
without its `live` feature, so the simulator and test scenarios answer with each node's mock and
no call leaves Studio; the Runtime makes the real calls. Graphs store the connector
configuration and the **name** of a secret; values come from the Runtime's environment
(`DONKA_SECRET_<NAME>`). Timeouts, retries and a circuit breaker are enforced by the handler, not
by rule authors. The node format is in `docs/artifact-format.md`.

## Decision log

Every decision a Runtime makes is sent to Studio in the background (`src/decision_log.rs` in the
Runtime): a bounded queue, batches, retries with backoff; when the queue is full, records are
dropped and counted rather than slowing answers. The Runtime authenticates with a
**decision-log token** an administrator issued for its environment, so a staging Runtime cannot
write production records. Studio stores a record only when its release was published to that
environment of its project. The feed format: [`decision-log-feed.md`](decision-log-feed.md).

`crates/decision-log` owns the records. Search fields stay readable; what the decision read and
answered is encrypted with AES-256-GCM (`DONKA_DECISION_LOG_KEY`). Every member can search;
opening and replaying a record are audited. Replay evaluates the record's input with its
release, connector nodes answering from the recorded trace (`ConnectorAdapter::replay`), and
says whether the result is identical. A worker purges records past
`DONKA_DECISION_LOG_RETENTION_DAYS`, one audit event per project purged.

## Explanations

When the installation sets `DONKA_EXPLAIN_URL`, any project member can ask Studio to explain a
logged decision in plain language (`crates/explain`). Studio removes the project's redacted
fields (dotted paths, owners list them in Settings) from the record's input, output and trace,
records `decision_record.explained` in the audit log, then sends the rest to the customer's own
LLM endpoint: Anthropic's Messages API or any OpenAI-compatible chat completions endpoint. The
explanation is written in the reader's language and only shown; the model has no tools and
nothing it says is stored or acted on. With the feature off, the endpoint answers `404` and the
web app shows no explain control.

## Security baseline

- Session cookies (argon2 password hashes) for Studio users. Owner / editor / viewer per project.
- Runtime access tokens are hashed, scoped to one environment, and rotatable.
- Audit and decision-log tables are append-only at the database level; decision records can
  only be deleted by the retention purge, which the table's trigger lets through.
- Decision records are encrypted at rest; decision-log tokens are hashed and scoped to one
  environment.
- Explanations are off by default; a record leaves Studio only without its project's redacted
  fields, and each time is audited.
- The browser never holds a Runtime token. Fieldkit calls the Runtime through the customer's backend.
