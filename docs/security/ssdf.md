# SSDF mapping

**Framework:** NIST SP 800-218 (SSDF) version 1.1. **Mapped:** 2026-11-09, against `develop`
at `dc3229d`. **Status:** self-assessed (DNK-73), like the rest of this dossier. Each practice
names how Donka does it and where a reader verifies it: repository rules (`AGENTS.md`,
`CONTRIBUTING.md`), CI (`.github/workflows/ci.yml` unless named), or a published page.

## Prepare the Organisation (PO)

| Practice | How Donka does it | Evidence |
| --- | --- | --- |
| PO.1 Define security requirements | Security and privacy are story-level criteria, not afterthoughts; the threat model allocates controls per story | `docs/backlog/`, [`threat-model.md`](threat-model.md) |
| PO.2 Implement roles and responsibilities | One maintainer merges; the product owner approves stories, plans and every merge | `CONTRIBUTING.md` (publishing workflow), story workflow in `AGENTS.md` |
| PO.3 Implement a supporting tool chain | Locked toolchains (Rust stable, Node 22, pnpm 10.33.0), pinned CI actions, Trivy + cargo audit + coverage gates | `.github/workflows/`, `AGENTS.md` §5, DNK-49 |
| PO.4 Define and use cryptography policy | AES-256-GCM sealing, Argon2 passwords, Ed25519 artifact signatures, key rotation paths | [`asvs.md`](asvs.md) V6, DNK-87 |
| PO.5 Promote adherence to secure coding | Engineering guides per stack, strict Clippy/lint/typecheck as merge gates | `docs/engineering/`, CI `rust`/`web` jobs |
| PO.6 (training) | Maintainer practice is documented rather than course-based; the guides carry the secure patterns | `docs/engineering/backend.md` §9, `frontend.md` |

## Protect the Software (PS)

| Practice | How Donka does it | Evidence |
| --- | --- | --- |
| PS.1 Protect code from tampering | Protected `develop`, one story branch, squash merges, no direct commits to `main`/`develop` | Branch rules + `CONTRIBUTING.md`; signed release artifacts (DNK-87) |
| PS.2 A mechanism for verifying software integrity | Signed artifacts verified by the Runtime; signed images are an owner release item, stated exactly | DNK-87; DNK-73 part 4 (SLSA preparation) |
| PS.3 Archive and protect each software release | Immutable versions and artifacts, content hashes, decision-log sealing, retention defaults | `crates/release`, `crates/decision-log`, install guides |

## Produce Well-Secured Software (PW)

| Practice | How Donka does it | Evidence |
| --- | --- | --- |
| PW.1 Design to minimise vulnerabilities | Deny-by-default, single-responsibility crates, contracts validated by schema | Principles, ADR-001…004, threat model |
| PW.2 Review design | Design decisions recorded as ADRs and story Decisions with the owner's answers | `docs/adr/`, story "Decisions" sections |
| PW.3 Verify third-party components | Locked dependencies, `cargo audit`, Trivy image scans, fork changes kept minimal and tested (SECURITY.md rule 3) | CI `rust`/`image` jobs; engine fork policy in threat model RA-1 |
| PW.4 Reuse existing, well-secured components | Adopted upstream components are inventoried with what Donka changed | `docs/engineering/references.md` |
| PW.5 Create source code securely | Memory-safe Rust/TypeScript only, generated code never hand-edited (OpenAPI types) | `AGENTS.md`, CONTRIBUTING rules |
| PW.6 Configure the build process securely | CI builds from locked manifests, offline bundle job, no secrets in images (checked) | CI `image` job (`check-image.sh`) |
| PW.7 Review and analyse human-readable code | Every change is a reviewed PR; the owner answers reviews before merge | GitHub PR history (e.g. #85–#95) |
| PW.8 Test executable code for vulnerabilities | Unit/integration/live-browser suites on every PR; ASVS self-assessment; penetration test by an independent firm is DNK-69 part 2 (owner item) | CI six jobs; [`penetration-test-scope.md`](penetration-test-scope.md) |
| PW.9 Configure software with secure defaults | Localhost publishing, strict CSP/HSTS, signed artifacts, MFA policy options | DNK-85, DNK-86, DNK-87, DNK-59 |

## Respond to Vulnerabilities (RV)

| Practice | How Donka does it | Evidence |
| --- | --- | --- |
| RV.1 Identify and confirm reported vulnerabilities | Private vulnerability reporting enabled per repository; `SECURITY.md` sets timelines | `SECURITY.md` rule 3 (DNK-69) |
| RV.2 Assess, prioritise and address | Findings enter the threat model with status (fixed/accepted, owner named) and the backlog as stories | Threat model §3 order table; DNK-83…87 are its output |
| RV.3 Analyse root causes | Post-fix regressions must fail before and pass after (a house rule with CI evidence) | `CONTRIBUTING.md`; examples in DNK-53/55 PRs |

Gaps are not hidden: penetration-test results (DNK-69 part 2), SLSA provenance and Scorecard
(DNK-73 part 4), and the incident rehearsal (DNK-50) are named in
[`asvs.md`](asvs.md) as exceptions or later parts, with owners.
