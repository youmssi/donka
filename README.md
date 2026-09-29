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
apps/web        Studio web app (Next.js static export, shadcn/Radix, jdm-editor)
crates/engine   The only code that calls zen-engine (DecisionRuntime trait + ZenRuntime)
crates/audit    Append-only audit log of every state change
crates/decision Decisions of a project and their drafts
crates/db       PostgreSQL pool, migrations, readiness
crates/identity Users, sign-in with lockout, sessions, invitations, password reset
crates/mail     Mailer trait and its SMTP implementation
crates/project  Projects, members and roles
crates/shared   Clock and pagination shared by the modules
migrations      PostgreSQL migrations (sqlx), forward-only
docs            Architecture, ADRs, roadmap, backlog, engineering guides
```

## Develop

Requirements: Rust stable, Node 22 + pnpm, Docker (for Postgres, MinIO and Mailpit).

```bash
docker compose up -d postgres minio mailpit
export DATABASE_URL=postgres://donka:donka@localhost:5432/donka
cargo test --workspace          # unit + integration tests (integration tests need DATABASE_URL)
cargo run -p donka-app          # Studio app on :8080, API under /api/v1, migrations applied at start
# Health: /api/v1/health · Readiness: /api/v1/ready · OpenAPI: /api/v1/openapi.json
```

Web app, in a second terminal (hot reload on :3000; `/api` is forwarded to the app on :8080, or to
`DONKA_DEV_API_ORIGIN`):

```bash
pnpm --dir apps/web install
pnpm --dir apps/web dev          # http://localhost:3000
```

First run: start the app with `DONKA_BOOTSTRAP_ADMIN_EMAIL=you@bank.example`,
`DONKA_PUBLIC_URL=http://localhost:3000` and `DONKA_COOKIE_SECURE=false` (local HTTP). It prints a
one-time link to choose your password; invitation and reset emails land in Mailpit
(http://localhost:8025).

To try the production setup, build the export and let the app serve it on one origin:
`pnpm --dir apps/web build && DONKA_WEB_DIR=apps/web/out cargo run -p donka-app`.

The API directly: every request that changes data must send the `x-donka-csrf: 1` header.

```bash
curl -s -c jar -X POST localhost:8080/api/v1/auth/sign-in -H 'x-donka-csrf: 1' \
  -H 'content-type: application/json' -d '{"email":"you@bank.example","password":"..."}'
curl -s -b jar -X POST localhost:8080/api/v1/simulate -H 'x-donka-csrf: 1' \
  -H 'content-type: application/json' \
  -d '{ "decisions": { "table": <a JDM graph> }, "key": "table", "context": { "input": 12 } }'
```

## License

Donka Studio is proprietary (see [LICENSE](LICENSE)). Open-source components keep their own
licenses; see [NOTICE](NOTICE).
