# Security questionnaire — pre-filled answers

**Purpose (DNK-73):** the questions a bank's security team asks a software supplier,
answered in advance, each answer pointing at checkable evidence in this dossier. Status
words are exact: *self-assessed* is never written as *certified*. Written 2026-11-09
against `develop` at `4ce8625`. [Version française](questionnaire.fr.md).

| # | Question | Answer | Evidence |
| --- | --- | --- | --- |
| 1 | What is the product and how is it deployed? | Decision-management software (Studio + Runtime + CLI) hosted on the bank's own servers with Docker; nothing is operated by Donka. | Install guides EN/FR |
| 2 | Do you hold ISO 27001 or SOC 2 certification? | No. Both certify an organisation over time, not software; the programme is a Phase 3 decision. What exists is a control-by-control mapping. | [Compliance matrices](compliance.md) |
| 3 | Has the application been security-tested? | Continuously: locked-dependency audits, image scanning and full test suites on every change; an OWASP ASVS 4.0.3 level 2 self-assessment with named exceptions. Independent penetration test: scope published, firm selection is with the customer programme (DNK-69 part 2). | [ASVS](asvs.md), [pentest scope](penetration-test-scope.md) |
| 4 | How is the software built and delivered? | NIST SSDF mapping; one story per reviewed branch; squash-merged only when six CI jobs pass. | [SSDF mapping](ssdf.md) |
| 5 | Are releases signed and verifiable offline? | Release artifacts are Ed25519-signed by the installation key and verified by the Runtime before use; image provenance (SLSA) is prepared and stated exactly as awaiting the first signed release (DNK-73 part 4). | DNK-87; install guides |
| 6 | How are vulnerabilities reported and fixed? | Private reporting per repository; timelines and supported versions in `SECURITY.md`; findings enter the threat model with an owner. | `SECURITY.md`, [threat model](threat-model.md) |
| 7 | Where does our data live? Who can access it? | Only on the bank's servers. Donka has no access, no telemetry, no hosted copy. | Architecture docs |
| 8 | How is personal data protected? | PII is marked in contracts, redacted in feeds/replays/explanations, retained by policy and deletable per person; card numbers are refused outright. | [Compliance matrices](compliance.md), [PCI scope](pci-dss-scope.md) |
| 9 | Does the product fall in PCI DSS scope? | No, by design: contracts holding or resembling a card number are refused at save time. | [PCI DSS scope](pci-dss-scope.md) |
| 10 | Authentication and access control? | Per-person accounts, Argon2 passwords, optional/required second step (TOTP or mailed code), project roles with four-eyes approvals, scoped expiring tokens. | [ASVS](asvs.md) V2–V4 |
| 11 | Encryption? | AES-256-GCM sealing for stored secrets and the decision log; Ed25519 artifact signatures; TLS at the bank's proxy; HSTS and a strict CSP in the browser. | [ASVS](asvs.md) V6/V9, DNK-85 |
| 12 | Auditability? | Immutable audit log and a sealed, verifiable decision log; every decision can be replayed and explained under redaction rules. | DNK-54, DNK-88 |
| 13 | Continuity and recovery? | Backup shipped with the installation; the timed restore/upgrade rehearsal (RPO/RTO) is DNK-50 and its results will be published here. | [Compliance matrices](compliance.md) (ISO 22301) |
| 14 | Accessibility? | WCAG 2.2 AA self-assessment with written limits (graph-editor keyboarding, target sizes). | [WCAG](wcag.md) |
| 15 | Which regulations does the mapping cover? | ISO/IEC 27001:2022 Annex A, SOC 2 criteria, GDPR, ISO 22301, EU AI Act high-risk duties, Malabo Convention, CEMAC/COBAC, UEMOA/BCEAO, Guinea, and pilot-country laws such as Cameroon 2024/017, each with owner (Donka/customer/shared) and status. | [Compliance matrices](compliance.md) |
| 16 | Can we verify all this ourselves? | Yes: every answer names a file, a CI job or a page that can be opened. The Trust page (DNK-66) will publish the dossier with SBOMs and the signing public key. | This folder |

Answers change in the same commit as the control they describe; an answer that becomes
false is a defect, reported through `SECURITY.md` like any other.
