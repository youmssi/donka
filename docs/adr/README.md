# Architecture decisions

| ADR | Decision |
|---|---|
| [001](001-separate-studio-and-runtime.md) | Studio and Runtime are separate; they share only release artifacts |
| [002](002-static-web-same-origin-session.md) | Web is a static export served by the app; httpOnly same-origin session |
| [003](003-single-engine-version-behind-a-trait.md) | zen-engine 2.0.1 everywhere, only behind `DecisionRuntime` |
| [004](004-ui-kit-shadcn-radix-with-jdm-editor.md) | shadcn/ui on Radix; jdm-editor unchanged |
| [005](005-tanstack-form.md) | TanStack Form + Zod for all forms |
| [006](006-fork-policy.md) | How the forks follow the playbook |
| [007](007-sessions-on-our-own-pool.md) | Sessions on Studio's own pool; hashed tokens; CSRF header |

## Product decisions (signed off 2026-09-28)

| Topic | Decision |
|---|---|
| Release storage | Shared disk volume on one server, the customer's S3 across machines, SILO as the documented self-hosted S3, Garage as the alternative (decided 2026-10-25) |
| Version history | Linear history per project + releases; branches in Stage 2 |
| Rule formats | Decision graphs only in Stage 1 (policy documents have no open-source editor) |
| AI explain | Off by default; calls the customer's own LLM endpoint; read-only |
| Decision log | Every production evaluation logged, encrypted, retention configurable |
| Rollback | Redeploying an approved release: instant for approvers, always audited |
| Approvals | One approver, never the author, gates production only |
| First customer | Bank or microfinance lender; starter pack: person scorecard, SME treasury evaluation |
| Languages | English and French |
| Licensing | Studio proprietary with a license key; Runtime and CLI MIT |
| Fieldkit | Keeps its name, "by Donka"; separate open-source repo; server-side Runtime calls only |
| Pricing structure | Annual license per deployment, tiered by environments and features |
| Workflow | Tickets `DNK-<n>`; story PRs squash-merged by their author once CI is green |
| Releases | release-please on `main` generates versions and changelogs (DNK-30); nothing published to registries under GoRules names |
| Projects | Administrators create projects and become owner; they are not implicit members of others; keys are immutable (DNK-7) |
| Repository visibility | `donka` is public until launch (free CI minutes), then made private; `donka-runtime` and `donka-cli` stay public (MIT) |

## Playbook adaptations

| Adaptation | Decision and guide |
|---|---|
| Separate authoring/control plane and serving/data plane | ADR-001; backend guide section 1 |
| Static Next.js export, no deployed Next server or BFF; same-origin httpOnly session | ADR-002 and ADR-007; frontend section 2, backend section 8 |
| Rust module crates and a single decision-engine boundary | ADR-003; backend section 1 |
| Registry UI with the upstream decision editor and shared design tokens | ADR-004; frontend sections 2 and 5 |
| Shared TanStack Form/Zod composition rather than the generic form-library example | ADR-005; frontend section 4 |
| Upstream fork structure/API paths, retained MIT notices and narrow changes | ADR-006; Runtime/CLI AGENTS and DONKA guides |

## Roadmap decisions confirmed 2026-10-20

- Phase 1 MVP: Studio + Runtime + CLI. Complete acceptance evidence, live feature tests and
  operational rehearsal before Phase 2.
- Phase 2: Fieldkit plus document-assisted rule creation, input/form/connector mapping,
  playground testing and chat edits. The owner sees proposed changes and a diff, then accepts
  before they are applied to the editable draft. Existing releases and approvals still apply.
- SSO/OIDC remains Phase 3 (moved to Phase 2 on 2026-10-25, below). The earlier Phase 1
  read-only explanation decision remains valid; Phase 2 authoring is a new capability with its
  architecture to be designed in that phase.
- Agree each phase and then proceed by story; unclear requirements and material scope changes
  require interactive alignment before implementation.
- Live Playwright verification follows every implemented feature. Chromium runs on each
  feature PR; Chromium, Firefox and WebKit run before release. UI coverage includes EN/FR,
  light/dark, desktop and 390 px.

## Roadmap decisions confirmed 2026-10-25

- Phase 1 adds a second sign-in step for local accounts (DNK-59) before the pilot.
- Single sign-on through OpenID Connect moves to Phase 2 (DNK-60). Donka ships no identity
  provider; authorization stays in Studio.
- Phase 2 adds analysis on recorded decisions (DNK-56, DNK-57, DNK-58) in a module that can be
  turned off, and the open-source `donka-scorecard` package (DNK-61) built on optbinning.
- Phase 1 also adds the engine upgrade (DNK-62), OAuth2 and mutual TLS connectors for the
  pilot's core banking system (DNK-63), secrets from files (DNK-65) and a five-minute install
  (DNK-64).
- Images on GitHub Container Registry, signed with cosign keys; upgrades are assisted and
  reversible (`donka upgrade`), never unattended in production.
- Product documentation in a `donka-docs` repository (Fumadocs), generated where possible and
  updated by every story (DNK-66).
- Phase 1 also gains a monthly-partitioned decision log (DNK-68), measured performance
  (DNK-67), image scanning in DNK-49, a security review with a penetration test (DNK-69), and
  RPO/RTO with Runtime availability in DNK-50. Phase 2 candidates add batch evaluation
  (DNK-70), protected characteristics (DNK-71) and Fieldkit on its own with XLSForm (DNK-72).
- Release storage: a shared disk volume on one server, the customer's S3 across machines,
  PGSTY SILO (maintained MinIO fork) as the documented self-hosted S3, Garage as the alternative.
- One language model configuration, chosen by the customer (their endpoint and key, hosted or
  on their own servers), serves explanations and the Phase 2 assistant.
- Python stays in `donka-scorecard`; it never enters Studio or the Runtime.
- Not added: a cache server (sessions and lockout live in PostgreSQL, rate limits in memory per
  instance), a feature-flag service (one installation per customer, configured by variables),
  an embedded observability stack (OpenTelemetry exports to the customer's), a vault client
  (secrets arrive as files) and a wiki server (documentation stays in the repositories).
- Open edges, closed control plane: the Runtime, CLI, connectors and `donka-scorecard` are open
  source; Studio is proprietary.

See the [delivery plan](../delivery-plan.md) for the complete scope and phase exit gates.
