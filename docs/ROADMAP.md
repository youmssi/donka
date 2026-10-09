# Roadmap

Stage 1 is the MVP for one self-hosted pilot (a bank or microfinance lender). Stories and
acceptance criteria: [`docs/backlog/stage-1.md`](backlog/stage-1.md). One story at a time, each
merged before the next.

Already on `develop` before the playbook was adopted: Cargo workspace, `crates/engine`
(`DecisionRuntime` + `ZenRuntime` on zen-engine 2.0.1), `apps/app` with health and simulate, CI,
docker-compose for PostgreSQL, MinIO and Mailpit.

## Stage 1 order

| # | Story | Repos |
|---|---|---|
| 1 | DNK-1 Work the same way in every Donka repo | S R C |
| 2 | DNK-2 API conventions | S |
| 3 | DNK-3 Persistent storage | S |
| 3b | DNK-25 Practices from the upstream codebases (docs) | S |
| 4 | DNK-12 Runtime on engine 2.0.1, own name | R |
| 4b | DNK-31 Local stack images and CI engine-drift check | S |
| 5 | DNK-4 Sign in | S |
| 6 | DNK-5 Invitations and password reset | S |
| 7 | DNK-6 Web foundation | S |
| 8 | DNK-7 Projects and members | S |
| 8b | DNK-33 People: invite and manage accounts | S |
| 9 | DNK-11 Audit log | S |
| 9b | DNK-34 Sidebar shell and one component system | S |
| 10 | DNK-8 Edit and simulate | S |
| 11 | DNK-9 Versions and diff | S |
| 12 | DNK-10 Test scenarios | S |
| 13 | DNK-13 Hashed per-environment tokens | R S |
| 14 | DNK-14 Releases and environments | S |
| 15 | DNK-15 Approval gate | S |
| 16 | DNK-16 Rollback | S |
| 17 | DNK-17 Connectors | R S |
| 18 | DNK-18 Decision log | R S |
| 19 | DNK-19 AI explain | S |
| 20 | DNK-20 rules-sync API | S |
| 21 | DNK-21 Donka CLI | C |
| 22 | DNK-22 Runtime rate limits | R |
| 23 | DNK-23 One-command install | S |
| 24 | DNK-24 Credit starter pack | S |
| 25 | DNK-37 One input contract for the rules and the form | S R |
| 26 | DNK-38 Forms pull and check the input contract | C |
| 26b | DNK-44 One form hook for Studio and Fieldkit | S |
| 27 | DNK-40 Rotate the decision-log key | S |
| 28 | DNK-43 Packs: import, duplicate, export | S |
| 29 | DNK-41 First-run onboarding | S |
| 30 | DNK-42 Wording that fits any industry | S |

Practices from the upstream codebases, scheduled next to the stories that need them:
DNK-26 `/version` (after DNK-4), DNK-28 container builds (before DNK-23), DNK-29 Storybook
(with DNK-6), DNK-27 OpenTelemetry (before DNK-23), DNK-30 generated changelogs (before the
first release PR), DNK-32 rate limits on sign-in and password reset (before the pilot).

## Stage 2: governance depth
Branches, N-eyes approvals, webhooks, GitSync, more connectors, Fieldkit Stage 1 (rendering
forms from the DNK-37 input contract).

## Stage 3: enterprise
SSO/OIDC, global roles, path coverage, policy-document editor, MCP bridge in Studio, SOC 2
program, Case Tracker, optional multi-tenant SaaS.
