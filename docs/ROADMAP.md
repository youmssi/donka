# Roadmap

Stage 1 is the MVP for one self-hosted pilot (a bank or microfinance lender). Items marked ✅ are
merged.

## Stage 1: pilot-ready

### A. Foundation
- ✅ A1 Monorepo, Cargo workspace, CI (fmt, clippy, tests)
- ✅ A2 `crates/engine`: `DecisionRuntime` trait + `ZenRuntime` on zen-engine 2.0.1, bundle-level evaluation with sub-decisions
- ✅ A3 `apps/app` skeleton with `/api/health` and `/api/simulate`
- A4 Postgres schema + sqlx migrations (users, projects, members, decisions, versions, releases, environments, deployments, approvals, audit)
- A5 Session auth (argon2), login / logout / password reset, first-admin bootstrap
- A6 Roles per project (owner / editor / viewer), enforced by one middleware

### B. Authoring
- B1 `apps/web`: Next.js static export, shadcn/Radix shell, EN/FR, light/dark
- B2 Editor page with jdm-editor (client-only, antd themed to match)
- B3 Decisions stored via the API, immutable versions on save, history + diff view
- B4 Test scenarios (input → expected output) run on save; results shown on approval

### C. Releases and approvals
- C1 Project-level releases (semver), artifact = zip + `.config/project.json`
- C2 Publish to MinIO/S3 per environment (staging, production)
- C3 Approval gate: production needs one approver who is not the author; email via SMTP
- C4 Rollback = redeploy an approved release (approver role, audited)
- C5 `rules-sync` API compatible with donka-cli targets (`main`, `commit:`, `release:`, `env:`)

### D. Runtime (youmssi/donka-runtime)
- D1 Rebrand, upgrade to zen-engine 2.0.1
- D2 Hashed, per-environment access tokens
- D3 Connector handler (custom-node adapter) + secrets from env/vault + timeouts/retries
- D4 Decision-log emitter (async, batched, retried)
- D5 Rate limiting

### E. Trust
- E1 Append-only audit log (database-enforced)
- E2 Decision log: ingest, search, replay, retention
- E3 AI explain (off by default, customer LLM endpoint, read-only)

### F. Ship
- F1 `docker compose up` for studio + runtime + postgres + minio
- F2 donka-cli rebrand, CI templates
- F3 Credit starter pack: person scorecard, SME treasury evaluation, reason codes, Fieldkit questionnaire
- F4 Install and operations guide (EN/FR)

## Stage 2: governance depth
Branches, N-eyes approvals, webhooks, GitSync, more connectors, notifications, Fieldkit Stage 1.

## Stage 3: enterprise
SSO/OIDC, global roles, path coverage, policy-document editor, MCP bridge in Studio, SOC 2 program,
Case Tracker, optional multi-tenant SaaS.
