# Decisions

Signed off on 28 Sep 2026. Change a row only with a dated note underneath it.

| # | Topic | Decision |
|---|---|---|
| D1 | Serving decisions | Separate **Donka Runtime** (fork of agent-public) and **Donka Studio**. They share only published release artifacts. Studio has no production evaluate endpoint. |
| D2 | Release storage | MinIO in Docker Compose, any S3-compatible store in production. Postgres holds history, not the served artifact. |
| D3 | Version history | Linear history per project + releases in Stage 1. Branches in Stage 2. |
| D4 | Rule formats | Decision graphs only in Stage 1. ZEN 2.0 policy documents have no open-source editor. |
| D5 | AI explain | Off by default. When enabled it calls the customer's own LLM endpoint. Read-only. |
| D6 | Decision log | Every production evaluation is logged (input, output, release, trace). Encrypted, retention is configurable. |
| D7 | Rollback | Redeploying an already approved release is instant for the approver role and always audited. |
| D8 | First customer | Bank or microfinance lender (retail and SME credit). Starter pack: person scorecard, SME treasury evaluation. |
| D9 | Languages | English and French at launch. |
| D10 | Licensing | Studio proprietary with a license key. Runtime and CLI stay MIT (forks of MIT code). |
| D11 | UI kit | Studio shell in shadcn/ui on **Radix** + Tailwind. jdm-editor used unchanged (antd inside the editor only), themed with the same tokens. Port the editor gradually after the MVP only if customers ask. |
| D12 | Fieldkit | Keeps its name, "by Donka". Separate open-source repo. Talks to the Runtime only server-side. |
| D13 | Repos | `youmssi/donka` (this monorepo), `youmssi/donka-runtime`, `youmssi/donka-cli`. |
| D14 | Pricing structure | Annual license per deployment, tiered by environments and features. Numbers set after the first pilot. |
| D15 | Engine version | zen-engine **2.0.1** pinned exactly everywhere Donka evaluates (Studio simulator, Runtime). |
| D16 | Approvals | One approver, never the author, gates production only. Staging is open to editors. |

## Known constraints we accept

- **Editor linting uses the expression grammar bundled in `@gorules/jdm-editor`** (currently
  zen-expression 0.55 in WASM). It only drives in-editor hints; every simulated or served result
  comes from zen-engine 2.0.1 on the server. We track upstream releases and bump together.
- **jdm-editor pulls in antd.** It is loaded only on the editor page, so the rest of Studio stays
  pure shadcn/Radix.
- **Engine evaluations are not `Send`** (function nodes embed QuickJS). `ZenRuntime` runs them on a
  pinned worker pool, the same approach the upstream editor and agent use.
