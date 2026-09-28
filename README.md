# Donka Studio

Donka is a decision management platform for credit and risk teams. Analysts build scoring rules
visually, test them, get them approved by a second person, and release them to a runtime that
customer systems call. Every production decision can be explained and replayed.

It is built on the open-source [ZEN engine](https://github.com/gorules/zen) (MIT).

## How the pieces fit

```
Applicant ─▶ Fieldkit form ─▶ Customer backend ──evaluate──▶ Donka Runtime ──▶ bureau / KYC APIs
                                                                 ▲    │
                                             poll + hot reload   │    │ decision log (async)
                                                                 │    ▼
                                                   Release storage ◀── Donka Studio (this repo)
                                                   (MinIO / S3)        web + app + Postgres
```

| Repo | What it is |
|---|---|
| **youmssi/donka** (this repo) | Studio: authoring, versions, tests, releases, approvals, audit, decision log, AI explain |
| [youmssi/donka-runtime](https://github.com/youmssi/donka-runtime) | Serves decisions from published releases (fork of `gorules/agent-public`) |
| [youmssi/donka-cli](https://github.com/youmssi/donka-cli) | Pulls releases into CI/CD, MCP bridge (fork of `gorules/cli`) |

Details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · Decisions: [docs/adr/](docs/adr/README.md) ·
Plan: [docs/ROADMAP.md](docs/ROADMAP.md) · Stories: [docs/backlog/](docs/backlog/stage-1.md)

**Contributing:** read [AGENTS.md](AGENTS.md) and [CONTRIBUTING.md](CONTRIBUTING.md) first. Work happens
on `dnk-<n>-<slug>` branches, squash-merged into `develop`.

## Repository layout

```
apps/app        Studio backend service (Rust, Axum)
apps/web        Studio web app (Next.js static export, shadcn/Radix, jdm-editor)   [DNK-6]
crates/engine   The only code that calls zen-engine (DecisionRuntime trait + ZenRuntime)
migrations      Postgres schema (sqlx)                                               [DNK-3]
docs            Architecture, ADRs, roadmap, backlog, engineering guides
```

## Develop

Requirements: Rust stable, Node 22 + pnpm, Docker (for Postgres and MinIO).

```bash
cargo test --workspace          # engine + app tests
cargo run -p donka-app          # Studio app on :8080
docker compose up -d postgres minio
```

Try the simulator endpoint:

```bash
curl -s localhost:8080/api/simulate -H 'content-type: application/json' -d @- <<'EOF'
{ "decisions": { "table": <paste a JDM graph> }, "key": "table", "context": { "input": 12 } }
EOF
```

## License

Donka Studio is proprietary (see [LICENSE](LICENSE)). Open-source components keep their own
licenses; see [NOTICE](NOTICE).
