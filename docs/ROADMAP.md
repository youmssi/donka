# Roadmap

Stage 1 is the MVP for one self-hosted pilot (a bank or microfinance lender). Stories and
acceptance criteria: [`docs/backlog/stage-1.md`](backlog/stage-1.md). One story at a time, each
merged before the next.

Phase scope, owner decisions and exit gates: [Delivery plan](delivery-plan.md).
Phase 1 is Studio + Runtime + CLI with a second sign-in step; Fieldkit, AI authoring and SSO are
Phase 2; global roles are Phase 3.

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
| 31 | DNK-45 Show and hide passwords | S |
| 32 | DNK-46 Environment cards fit phones | S |
| 33 | DNK-29 Shared component previews and browser checks | S |
| 34 | DNK-47 Agreed delivery plan and foundation evidence | S |
| 35 | DNK-48 Live feature-test foundation; extend per feature group | S R C |
| 35b | DNK-51 Identity, projects and permission browser coverage | S |
| 35c | DNK-52 Editor, contracts, versions and scenarios | S R |
| 35d | DNK-53 Approval, rollback, Runtime and connectors | S R |
| 35e | DNK-54 Decision records, explanations and audit | S R |
| 35f | DNK-55 Packs, onboarding and CLI integration | S C |
| 36 | DNK-59 Two-step sign-in with an authenticator app | S |
| 37 | DNK-62 Bring the decision engine up to date | R S |
| 38 | DNK-63 Connectors that authenticate like the core banking system | R S |
| 39 | DNK-65 Read secrets from files | S R |
| 40 | DNK-64 A first decision five minutes after install | S R |
| 41 | DNK-49 Enforce coverage and dependency-security gates | S R C |
| 42 | DNK-50 Isolated backup, restore and upgrade rehearsal | S R C |

Practices from the upstream codebases, scheduled next to the stories that need them:
DNK-26 `/version` (after DNK-4), DNK-28 container builds (before DNK-23), DNK-27 OpenTelemetry
(before DNK-23), DNK-30 generated changelogs (before the first release PR), DNK-32 rate limits
on sign-in and password reset (before the pilot).

## Stage 2: governance depth
Fieldkit Stage 1 (forms from DNK-37), document-assisted AI rule creation and chat editing,
connector/field mappings and the owner playground. Proposed edits require an owner-reviewed
diff before application to a draft; normal release approval remains. Branches, N-eyes approvals,
webhooks, GitSync and more connectors remain in this phase. Agreed candidates in
[`backlog/stage-2.md`](backlog/stage-2.md): DNK-60 single sign-on, DNK-56 swap-in/swap-out
on past decisions, DNK-57 decision mix and stability, DNK-58 the same in the playground, and
DNK-61 the open-source `donka-scorecard` package. Define and agree the detailed
story order at phase entry; see the [approved scope](delivery-plan.md#phase-2--assisted-creation-fieldkit-and-governance).

## Stage 3: enterprise
Global roles, path coverage, policy-document editor, MCP bridge in Studio, SOC 2
program, Case Tracker, optional multi-tenant SaaS.
