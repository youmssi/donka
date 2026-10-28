# Capability map

What a serious business-rules platform and a serious credit-scoring platform must offer, and
where each capability is in Donka: done (story merged), planned (story and phase), a later
candidate, or deliberately left to the customer's own systems. Reviewed on 2026-10-25 against
the backlog, the [roadmap](ROADMAP.md) and the [delivery plan](delivery-plan.md). A capability
with no row here is not covered; add it before promising it.

Status: **Done** · **P1**, **P2**, **P3** (phase) · **Later** (candidate, not agreed) ·
**Customer** (stays in the customer's systems, by design).

## Rules platform (BRMS)

| Capability | Status | Where |
|---|---|---|
| Visual decision graphs, decision tables, expressions, functions | Done | DNK-8 (`@gorules/jdm-editor`, zen-engine) |
| Simulation with mocked external calls | Done | DNK-8, DNK-17 |
| Versions, diff, restore | Done | DNK-9 |
| Test scenarios run on every version | Done | DNK-10 |
| Input contracts: types, ranges, personal data, Runtime validation | Done | DNK-37 |
| Releases, environments, second-person approval, rollback | Done | DNK-14, DNK-15, DNK-16 |
| Audit log with export | Done | DNK-11 |
| Starter packs; import, duplicate, export | Done | DNK-24, DNK-43 |
| Table verification: gaps, overlaps, unreachable rows | Done | DNK-76 |
| Up-to-date engine, upgraded on replay evidence | Done | DNK-62 (zen-engine 2.1.6) |
| Impact of a change on past decisions (swap-in/out) | P2 | DNK-56, DNK-58 |
| Branches, several approvers, webhooks, Git sync | P2 | Delivery plan, Phase 2 |
| Document-assisted authoring and chat edits accepted as a diff | P2 | Delivery plan, Phase 2 |
| Policies effective from a date | P2 | DNK-78 |
| Rules document for auditors (PDF) | P2 | DNK-79 |
| Batch evaluation of a portfolio | P2 | DNK-70 |
| Decision-path coverage, policy-document editor, global roles | P3 | Delivery plan, Phase 3 |
| Process orchestration (BPMN), case management, manual review queue | Customer / P3 | The lender's loan system; Case Tracker in Phase 3 |
| Event-stream integration (Kafka) | Later | Not yet a candidate story |

## Credit scoring

| Capability | Status | Where |
|---|---|---|
| Expert scorecards: points, bands, decisions, reason codes | Done | Packs (DNK-24, DNK-43) |
| Bureau, KYC and AML calls with timeouts, retries, circuit breaker | Done | DNK-17 |
| Core banking authentication: OAuth2 client credentials, mutual TLS | Done | DNK-63 |
| Every decision logged, encrypted, searchable, replayable | Done | DNK-18, DNK-40 |
| Plain-language explanation, customer-chosen model | Done | DNK-19 |
| Statistical scorecards (binning, WoE, scaling) proven equal in Studio | P2 | DNK-61 (`donka-scorecard`) |
| Decision mix and population stability (SSI/CSI) | P2 | DNK-57 |
| Protected characteristics kept out of decisions | P2 | DNK-71 |
| Repayment performance: default rate by band, Gini/KS in production | Later | Needs labelled outcomes from the core banking system |
| Shadow evaluation, then champion/challenger | Later | Needs repayment performance first |
| Field capture tied to the decision contract | Done (contracts, CLI) · P2 (Fieldkit) | DNK-37, DNK-38, Fieldkit baseline, DNK-72 |
| Public pre-qualification without documents | P2 | DNK-80 |
| Intake queue: assignment, call log, overdue alerts | P2 | DNK-81 (first slice of the Case Tracker) |
| Verified evaluation, declared-versus-verified gaps, screening gate, overrides | P2 | DNK-82 |

## Security, privacy and compliance

| Capability | Status | Where |
|---|---|---|
| Sessions, lockout, rate limits, CSRF, hashed tokens | Done | DNK-4, DNK-5, DNK-13, DNK-22, DNK-32 |
| Two-step sign-in | Done | DNK-59 |
| Single sign-on with the customer's identity provider | P2 | DNK-60 |
| Secrets from files; key rotation | Done | DNK-65 · DNK-40 |
| Rights of individuals: access, anonymisation, contest | Done | DNK-75 |
| Dependency and image scanning, coverage gates | P1 | DNK-49 |
| Threat model and external penetration test | P1 | DNK-69 |
| OWASP ASVS L2, NIST SSDF, SLSA L3, OpenSSF, CIS, PCI DSS out of scope | P1 | DNK-73 |
| Compliance matrices: ISO/IEC 27001, SOC 2, GDPR, ISO 22301, EU AI Act, WCAG 2.2, Malabo, CEMAC, UEMOA, Guinea | P1 | DNK-73 |
| Company certification: ISO/IEC 27001, SOC 2 Type II | P3 | Delivery plan, Phase 3 |

## Installation, operation and support

| Capability | Status | Where |
|---|---|---|
| Compose installation, smoke test, version endpoint, OpenTelemetry | Done | DNK-23, DNK-26, DNK-27 |
| Signed images, offline bundle, five-minute demo | P1 | DNK-64 |
| Guided setup with chosen modules; `donka-admin` doctor and support bundle | P1 | DNK-74 |
| Assisted, reversible upgrade, never unattended in production | P1 | DNK-64 |
| Partitioned decision log for long retention | Done | DNK-68 |
| Measured performance with published targets | P1 | DNK-67 |
| Backup, restore, RPO/RTO, Runtime availability | P1 | DNK-50 |
| Signed licence file; the Runtime never blocked | P1 | DNK-77 |
| Product documentation, Trust and Performance pages, API reference | P1 | DNK-66 |
| Pilot integration questionnaire | Done | [pilot-questionnaire.md](pilot-questionnaire.md) |
| Runtime client SDKs; "Ask" search in the documentation | Later | Stage 2 candidates |

## Deliberately not built

A cache server, a feature-flag service, an embedded observability stack, a vault client, a wiki
server, an identity provider shipped with Donka and a desktop installer. The reasons are in the
roadmap decisions of [docs/adr/README.md](adr/README.md).
