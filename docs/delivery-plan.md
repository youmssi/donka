# Delivery plan

Product-owner decisions confirmed on 2026-10-20 and 2026-10-25. This page adds the delivery gates and future
scope to the existing [roadmap](ROADMAP.md); the [backlog](backlog/stage-1.md) defines each story.

## Phase 1 — pilot MVP

Studio, Runtime and CLI, for one self-hosted customer installation. Complete this phase before
starting Phase 2. The existing read-only decision explanation remains part of Phase 1.

Remaining order:

1. **DNK-47:** record the agreed scope and testing policy; reconcile the foundation acceptance
   lists with [exact evidence](engineering/foundation-evidence.md).
2. **DNK-48:** put live feature tests in the repository with isolated fixtures, a repeatable
   environment and CI. Begin with authentication and the core project-to-Runtime journey.
   Extend coverage through DNK-51 to DNK-55, separate stories per feature group: permissions, editor/contracts,
   versions/scenarios, approvals/rollback, tokens/connectors, records/explanations/audit,
   packs/onboarding and CLI sync/contracts. Fix defects blocking the current journey in its
   story; record unrelated defects in separate stories.
3. **DNK-59** (confirmed 2026-10-25): a second sign-in step with an authenticator app and
   recovery codes, required by configuration for administrators and owners.
4. **DNK-62, DNK-63, DNK-65, DNK-64** (confirmed 2026-10-25): the decision engine brought up
   to date with replay evidence; connectors with OAuth2 client credentials and mutual TLS for the
   pilot's core banking system; secrets read from files; published images, a prepared
   installation and a demo that reach a first decision within five minutes. DNK-68 partitions
   the decision log first, before any pilot data exists; DNK-67 then measures performance
   against published targets. Also confirmed 2026-10-25: DNK-76 table verification after the
   engine upgrade; DNK-75 the rights of the people decisions are about (access, anonymisation,
   contest) after partitioning; DNK-77 a signed licence file; DNK-74 a guided command-line and
   browser setup with chosen modules, `donka-admin doctor` and a support bundle, after DNK-64.
5. **DNK-49:** enforce the coverage and dependency-security gates already required by the
   engineering guides, plus image scanning. Their current absence is an audit finding, not a
   passing check. **DNK-69** follows: a threat model and an external penetration test; then **DNK-73**, a
   trust dossier (OWASP ASVS level 2 self-assessment, NIST SSDF, SLSA level 3, OpenSSF, CIS,
   PCI DSS out of scope by design, and compliance matrices for ISO/IEC 27001, SOC 2, GDPR,
   ISO 22301, the EU AI Act, WCAG 2.2, the Malabo Convention and the CEMAC, UEMOA and Guinea
   rules) published with the load-test results on the documentation site.
6. **DNK-50:** rehearse installation, database/bucket/secret backup and restore, upgrade and
   recovery in isolation; verify the restored system through the full smoke journey; state and
   meet recovery point and time objectives; keep Runtimes answering during restarts.
7. **DNK-66** (confirmed 2026-10-25): the product documentation site, built from the
   repositories, versioned in English and French, covering integration, operations and security.
8. Close every applicable feature-test gap, confirm customer policy values and the pilot target,
   then obtain the deployment decision and follow the release workflow.

Phase exit: acceptance evidence is current, business journeys pass against running services,
all required CI gates pass, recovery is demonstrated, and pilot acceptance criteria are agreed.
The component-preview suite supports this evidence; it does not cover the full application.

## Phase 2 — assisted creation, Fieldkit and governance

Fieldkit and AI authoring belong together in this phase. Define individual stories and the
architecture before implementation; no AI-authoring service or Fieldkit repository is created
by this planning change. The approved product workflow is:

1. A project document library holds procedures and policies with versions and page/section
   references. Document access follows project permissions.
2. The assistant drafts decision graphs, tables, expressions, input contracts and scenarios
   grounded in those sources. Missing or conflicting requirements become questions.
3. Each input identifies its source: a Fieldkit field, the customer's core banking system or a
   third-party connector. Connector contracts/mappings use supplied API documentation;
   unresolved endpoints, credentials and business rules remain explicit questions.
4. Fieldkit form definitions, EN/FR labels, validation and mappings use the same input contract
   as the rules and the Runtime. Runtime credentials stay on the customer's backend.
5. A playground combines the form, rules and controlled connector responses. It is a view over
   the existing simulation (one evaluation path through `DecisionRuntime`), never a second one:
   confirmed 2026-10-25 as a mode of the editor's simulator panel (form input, proposed diff
   side by side), not a separate page. The owner can
   inspect expected results, traces, boundary cases and source references.
6. Chat can explain and revise rules, forms and mappings. It first presents proposed changes
   and a diff. **The owner accepts before anything is applied to the editable project draft.**
   Accepted edits keep the existing version, release and production-approval workflow.

Validate accuracy through source references, schema/engine checks, scenarios and owner review.
Use compatible shadcn chat/message/attachment components; conversation history, source
provenance and proposal acceptance are application behavior that must be implemented and tested.
The model helps author rules; the Runtime continues evaluating the released rules.

The previously planned branches, multi-person approvals, webhooks, GitSync and additional
connectors remain in Phase 2. Agree their story order at phase entry; do not silently grow
Phase 1 to include them.

Confirmed 2026-10-25, see [Stage 2 stories](backlog/stage-2.md):

- **Single sign-on** moves here from Phase 3 (DNK-60): OpenID Connect to the customer's
  identity provider, tested against Keycloak; Studio still decides permissions; no identity
  provider ships with Donka.
- **Analysis on recorded decisions**, a module that can be turned off: swap-in/swap-out of a
  release in the approval review (DNK-56), decision mix and population stability (DNK-57),
  then the same swap analysis in the playground (DNK-58).
- **`donka-scorecard`** (DNK-61), a separate Apache-2.0 Python package on optbinning that
  writes statistical scorecards as packs; its scenarios prove Studio computes the same score.
- Repayment outcomes, shadow and champion/challenger evaluation and passkeys stay later
  candidates until agreed; core banking connector authentication moved to Phase 1 (DNK-63).

## Phase 3 — enterprise

Global roles, decision-path coverage, the policy-document editor, Studio MCP
bridge, Case Tracker, the ISO/IEC 27001 and SOC 2 Type II programme for the company (started
when Donka operates a hosted service or a customer requires it, with third-party verification
of the ASVS assessment) and optional multi-tenant SaaS remain here.
These are roadmap themes requiring their own acceptance criteria and phase-entry agreement.
Document ingestion for Phase 2 authoring does not imply delivery of the Phase 3 policy editor.

## Alignment and verification for every story

- Agree each phase, then proceed one story at a time inside it. Ask interactive questions with
  options and a recommendation before coding an unclear requirement or material scope change.
- Branch from current `develop`, open a draft PR, implement and verify the acceptance criteria.
  Merge the story before starting the next; retain the existing Conventional Commit/Refs rules.
- After implementing a feature, run Playwright against the actual running application and its
  relevant services. Save the test in the repository; use isolated synthetic data and controlled
  external endpoints. Test relevant success, validation, permission and failure paths.
- Every UI change is checked in EN/FR, light/dark, desktop and 390 px. Chromium runs on each
  feature PR; run the regression suite in Chromium, Firefox and WebKit before release.
- Use user-facing locators, waiting assertions and real readiness conditions. Capture useful
  traces/screenshots on failure; protect credentials and personal data. Fix the cause of a
  failure instead of skipping it or retrying into green.
- API, CLI, migration and operational behavior also needs the corresponding integration or
  command tests. Documentation-only changes have document/link checks and required CI;
  unchanged application behavior does not need a new browser journey.
- Mark ready and squash-merge only after all required checks pass and reviews are answered.
  Report what passed and any limits on the evidence.

Studio stays public during development by the existing decision. The launch visibility change
and production deployment remain separate owner decisions.
