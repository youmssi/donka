# Delivery plan

Product-owner decisions confirmed on 2026-10-20. This page adds the delivery gates and future
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
3. **DNK-49:** enforce the coverage and dependency-security gates already required by the
   engineering guides. Their current absence is an audit finding, not a passing check.
4. **DNK-50:** rehearse installation, database/bucket/secret backup and restore, upgrade and
   recovery in isolation; verify the restored system through the full smoke journey.
5. Close every applicable feature-test gap, confirm customer policy values and the pilot target,
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
5. A playground combines the form, rules and controlled connector responses. The owner can
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

Confirmed 2026-10-25: rule-impact analysis on recorded decisions (DNK-56) and input drift
monitoring (DNK-57) are Phase 2 candidates, built on the decision log. Repayment outcomes,
shadow/champion-challenger evaluation and a separate statistical scorecard package stay later
candidates until agreed. See [Stage 2 stories](backlog/stage-2.md).

## Phase 3 — enterprise

SSO/OIDC and global roles, decision-path coverage, the policy-document editor, Studio MCP
bridge, Case Tracker, the SOC 2 readiness program and optional multi-tenant SaaS remain here.
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
