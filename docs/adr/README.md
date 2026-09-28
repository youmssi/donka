# Architecture decisions

| ADR | Decision |
|---|---|
| [001](001-separate-studio-and-runtime.md) | Studio and Runtime are separate; they share only release artifacts |
| [002](002-static-web-same-origin-session.md) | Web is a static export served by the app; httpOnly same-origin session |
| [003](003-single-engine-version-behind-a-trait.md) | zen-engine 2.0.1 everywhere, only behind `DecisionRuntime` |
| [004](004-ui-kit-shadcn-radix-with-jdm-editor.md) | shadcn/ui on Radix; jdm-editor unchanged |
| [005](005-tanstack-form.md) | TanStack Form + Zod for all forms |
| [006](006-fork-policy.md) | How the forks follow the playbook |

## Product decisions (signed off 2026-09-28)

| Topic | Decision |
|---|---|
| Release storage | MinIO in Docker Compose, any S3-compatible store in production |
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
