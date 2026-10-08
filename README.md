![Donka Studio](.github/assets/banner.svg)

<h1 align="center">Donka Studio</h1>

<p align="center">
    Build, test, approve and release credit decisions, then explain every one of them
</p>

<p align="center">
    <a href="https://github.com/youmssi/donka/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/youmssi/donka/ci.yml?branch=develop&label=CI" alt="CI"/></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-proprietary-555" alt="License: proprietary"/></a>
    <img src="https://img.shields.io/badge/rust-stable-b7410e?logo=rust&logoColor=white" alt="Rust stable"/>
    <img src="https://img.shields.io/badge/Next.js-static%20export-000?logo=nextdotjs&logoColor=white" alt="Next.js static export"/>
    <img src="https://img.shields.io/badge/PostgreSQL-16-4169e1?logo=postgresql&logoColor=white" alt="PostgreSQL 16"/>
    <img src="https://img.shields.io/badge/zen--engine-2.0.1-1d4f91" alt="zen-engine 2.0.1"/>
    <img src="https://img.shields.io/badge/i18n-English%20%C2%B7%20Fran%C3%A7ais-1d4f91" alt="English and French"/>
</p>

<p align="center">
    <a href="docs/ARCHITECTURE.md">Architecture</a> ·
    <a href="docs/ROADMAP.md">Roadmap</a> ·
    <a href="docs/backlog/stage-1.md">Backlog</a> ·
    <a href="docs/adr/README.md">Decisions</a> ·
    <a href="CONTRIBUTING.md">Contributing</a>
</p>

## Introduction

Donka is a decision management platform for credit and risk teams at banks and microfinance
lenders. Analysts build scoring rules visually, test them, get them approved by a second person
and release them to a Runtime that customer systems call. Every production decision can be
found, explained in plain language and replayed.

Donka Studio is the control plane: the web app where that work happens and the service behind
it. It is self-hosted, one installation per customer, and built on the open-source
[ZEN engine](https://github.com/gorules/zen) (MIT).

## Features

- **Visual authoring**: decision graphs, decision tables and expressions in the JDM editor, with
  autosave and conflict detection
- **Simulation and test scenarios**: run a decision on sample input, save it as a scenario, and
  see every scenario's result each time a version is saved
- **Versions and diff**: save versions, compare any two side by side, restore an older one
- **Releases and environments**: frozen, semantically versioned releases deployed to `staging`
  and `production`, with per-environment Runtime tokens
- **Four-eyes approvals**: production needs a second person, who reviews the diff and the test
  results; nobody approves their own release
- **Rollback**: put a previous release back live in one step, audited
- **Connectors**: call a credit bureau, KYC or AML provider from a decision, with mocks in Studio
  and secrets kept on the Runtime
- **Decision log and replay**: every production decision, encrypted at rest, searchable by
  reference, outcome or date, and replayable against its release
- **Plain-language explanations**: an answer in English or French from your own LLM endpoint,
  with sensitive fields kept out
- **Audit log**: every change, by whom and when, filterable and exportable to CSV
- **CI/CD**: pipelines pull release artifacts with read-only CI tokens and the
  [Donka CLI](https://github.com/youmssi/donka-cli)

## How the pieces fit

```
Applicant ─▶ Fieldkit form ─▶ Customer backend ──evaluate──▶ Donka Runtime ──▶ bureau / KYC APIs
                                                                 ▲    │
                                             poll + hot reload   │    │ decision log (async)
                                                                 │    ▼
                                                   Release storage ◀── Donka Studio (this repo)
                                                   (MinIO / S3)        web + app + Postgres
                                                         │
                                                         └──▶ CI/CD pipelines (Donka CLI)
```

| Repository | What it is |
| --- | --- |
| **youmssi/donka** (this repo) | Studio: authoring, tests, releases, approvals, audit, decision log, explanations |
| [youmssi/donka-runtime](https://github.com/youmssi/donka-runtime) | Serves decisions from published releases (fork of `gorules/agent-public`, MIT) |
| [youmssi/donka-cli](https://github.com/youmssi/donka-cli) | Pulls releases into CI/CD pipelines (fork of `gorules/cli`, MIT) |

## Quick start

Requirements: Rust stable, Node 22 with pnpm, Docker (Postgres, MinIO and Mailpit).

```bash
docker compose up -d postgres minio mailpit
export DATABASE_URL=postgres://donka:donka@localhost:5432/donka

DONKA_BOOTSTRAP_ADMIN_EMAIL=you@bank.example DONKA_PUBLIC_URL=http://localhost:3000 \
DONKA_COOKIE_SECURE=false cargo run -p donka-app      # API on :8080 under /api/v1

pnpm --dir apps/web install && pnpm --dir apps/web dev  # web app on http://localhost:3000
```

On first start the app prints a one-time link to choose your password. Invitation and reset
emails land in Mailpit (http://localhost:8025). Every setting is listed in
[.env.example](.env.example).

<details>
<summary>More: production build, health endpoints, calling the API</summary>

- **One origin, as in production**: `pnpm --dir apps/web build && DONKA_WEB_DIR=apps/web/out cargo run -p donka-app`
- **Health** `/api/v1/health` · **Readiness** `/api/v1/ready` · **OpenAPI** `/api/v1/openapi.json`
- **The API directly**: every request that changes data sends the `x-donka-csrf: 1` header.

```bash
curl -s -c jar -X POST localhost:8080/api/v1/auth/sign-in -H 'x-donka-csrf: 1' \
  -H 'content-type: application/json' -d '{"email":"you@bank.example","password":"..."}'
curl -s -b jar -X POST localhost:8080/api/v1/simulate -H 'x-donka-csrf: 1' \
  -H 'content-type: application/json' \
  -d '{ "decisions": { "table": <a JDM graph> }, "key": "table", "context": { "input": 12 } }'
```

</details>

## Documentation

| Page | What it covers |
| --- | --- |
| [Architecture](docs/ARCHITECTURE.md) | Modules, data flow, security baseline |
| [Artifact format](docs/artifact-format.md) | What Studio publishes for the Runtime (`.config/project.json`) |
| [Decision-log feed](docs/decision-log-feed.md) | How Runtimes send decisions to Studio |
| [Rules sync](docs/rules-sync.md) | How CI pipelines pull release artifacts |
| [Engineering guides](docs/engineering/) | Principles, backend and frontend conventions |
| [Decisions (ADRs)](docs/adr/README.md) · [Roadmap](docs/ROADMAP.md) · [Backlog](docs/backlog/stage-1.md) | Why, what next, and the stories |

## Contributing

Read [AGENTS.md](AGENTS.md) and [CONTRIBUTING.md](CONTRIBUTING.md) first. Each story gets a
`dnk-<n>-<slug>` branch, squash-merged into `develop`; the repository map is in
[AGENTS.md](AGENTS.md#8-repository-map).

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked                      # integration tests need DATABASE_URL
pnpm --dir apps/web lint && pnpm --dir apps/web typecheck && pnpm --dir apps/web test
```

## License

Donka Studio is proprietary, see [LICENSE](LICENSE). Open-source components keep their own
licenses, see [NOTICE](NOTICE).
