# Compliance matrices

**Status:** self-assessed mapping (DNK-73), written 2026-11-09. Donka is software the
customer hosts; no matrix here is a certification. Each requirement names its evidence,
who is responsible (**Donka** = the software and its documents, **Customer** = the bank
operating it, **Shared**) and its status: **Covered** (the software provides it),
**Customer** (only the operating bank can do it; the software does not block it),
**Partial** (named limit), **N/A** (with the reason). Gaps name their owner or story.

Frameworks: ISO/IEC 27001:2022 Annex A · SOC 2 trust services criteria · GDPR
(Regulation (EU) 2016/679) · ISO 22301:2019 · EU AI Act (Regulation (EU) 2024/1689) ·
Malabo Convention (AU Convention on Cyber Security and Personal Data Protection, adopted
27 June 2014, in force 8 June 2023) · CEMAC/COBAC and UEMOA/BCEAO banking supervision ·
Cameroon Law No. 2024/017 of 23 December 2024 on personal data protection · Guinea.

## ISO/IEC 27001:2022 Annex A

Donka is not the bank's ISMS. Controls about the customer's organisation stay Customer;
the software rows state what Donka provides toward them.

### A.5 Organisational (37)

| Control | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| A.5.1 Policies for information security | Customer | Customer | Bank ISMS policy; Donka ships `SECURITY.md` and this dossier |
| A.5.2 Roles and responsibilities | Shared | Covered | Instance administrators vs project roles (DNK-51); bank names its own owners |
| A.5.3 Segregation of duties | Shared | Covered | Four-eyes approvals, separate owner/approver (DNK-52/53) |
| A.5.4 Management responsibilities | Customer | Customer | — |
| A.5.5 Contact with authorities | Customer | Customer | — |
| A.5.6 Contact with special interest groups | Customer | Customer | — |
| A.5.7 Threat intelligence | Shared | Partial | Threat model maintained per story; no external intel feed (maintainer triage) |
| A.5.8 Security in project management | Donka | Covered | Security criteria in every story; AGENTS.md workflow |
| A.5.9 Inventory of information assets | Shared | Partial | SBOM + data inventory in dossier; bank inventories its deployment |
| A.5.10 Acceptable use | Customer | Customer | — |
| A.5.11 Return of assets | Customer | Customer | — |
| A.5.12 Classification of information | Shared | Partial | PII marking + redaction; written classification is ASVS EX-1 |
| A.5.13 Labelling | Shared | Partial | Same as A.5.12 |
| A.5.14 Information transfer | Shared | Covered | TLS at the bank proxy; connector rules (DNK-84) |
| A.5.15 Access control policy | Shared | Covered | Deny-by-default, project roles; bank sets its accounts |
| A.5.16 Identity management | Shared | Covered | Per-person accounts, invitations, deactivation (DNK-51) |
| A.5.17 Authentication information | Shared | Covered | Argon2, MFA options (DNK-59); bank enforces its policy |
| A.5.18 Access rights | Shared | Covered | Role changes audited; periodic review is the bank's process |
| A.5.19 Supplier relationships | Customer | Customer | Donka is the supplier; this dossier is the evidence offered |
| A.5.20 Supplier agreements | Customer | Customer | Contractual; `SECURITY.md` commitments |
| A.5.21 ICT supply chain | Donka | Covered | Locked dependencies, audits, signed artifacts (DNK-87) |
| A.5.22 Monitoring of supplier services | Customer | Customer | Bank monitors its deployment (doctor, audit log) |
| A.5.23 Cloud services | Shared | N/A | On-premise software; cloud use is the bank's choice and policy |
| A.5.24 Incident management planning | Shared | Partial | `SECURITY.md` + runbooks; recovery rehearsal run end to end (DNK-50; ASVS EX-2 closed) |
| A.5.25 Assessment of security events | Customer | Customer | Bank SOC process; audit log is the software input |
| A.5.26 Response to incidents | Shared | Partial | Same as A.5.24 |
| A.5.27 Learning from incidents | Customer | Customer | — |
| A.5.28 Collection of evidence | Shared | Covered | Immutable audit log, sealed decision log (DNK-54) |
| A.5.29 Continuity during disruption | Shared | Partial | Rehearsal run (DNK-50): recovery point 24 h (daily backup), recovery time target 1 h, measured 24 s on a demonstration-sized install |
| A.5.30 ICT readiness for continuity | Shared | Partial | Same as A.5.29; see ISO 22301 matrix |
| A.5.31 Legal and regulatory requirements | Shared | Covered | This page; bank's counsel confirms its own duties |
| A.5.32 Intellectual property | Donka | Covered | Licences inventoried in `docs/engineering/references.md` |
| A.5.33 Protection of records | Shared | Covered | Retention defaults (decision log 5 y), immutability |
| A.5.34 Privacy and PII protection | Shared | Covered | GDPR and regional matrices below; people-rights (DNK-75) |
| A.5.35 Independent review | Shared | Partial | ASVS/SSDF are self-assessed; pentest is DNK-69 part 2 |
| A.5.36 Compliance with policies | Donka | Covered | CI gates enforce the repository rules on every change |
| A.5.37 Documented operating procedures | Donka | Covered | Install/operation guides EN/FR |

### A.6 People (8)

| Control | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| A.6.1 Screening | Customer | Customer | Bank HR; Donka maintainer is the named owner |
| A.6.2 Terms of employment | Customer | Customer | — |
| A.6.3 Awareness and training | Customer | Customer | Guides and tours in the product; bank trains its staff |
| A.6.4 Disciplinary process | Customer | Customer | — |
| A.6.5 Responsibilities after termination | Shared | Covered | Deactivation revokes access and sessions (DNK-51) |
| A.6.6 Confidentiality agreements | Customer | Customer | — |
| A.6.7 Remote working | Customer | Customer | Software is network-agnostic behind the bank proxy |
| A.6.8 Event reporting by personnel | Shared | Covered | `SECURITY.md` reporting address and timelines |

### A.7 Physical (14)

All fourteen controls (perimeters, entry, equipment, cabling, clear desk and the rest)
are **Customer**: Donka runs in the bank's data centre. Status: Customer, no software
obstacle; offline media handling for the offline bundle is described in the install
guides.

### A.8 Technological (34)

| Control | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| A.8.1 User endpoints | Customer | Customer | Browsers of bank staff |
| A.8.2 Privileged access | Shared | Covered | Instance-admin separation, audited (DNK-51) |
| A.8.3 Information access restriction | Donka | Covered | Project scoping on every query; cross-project tests |
| A.8.4 Access to source code | Donka | Covered | Protected branches; releases signed (DNK-87) |
| A.8.5 Secure authentication | Donka | Covered | ASVS V2/V3; MFA policy (DNK-59) |
| A.8.6 Capacity management | Shared | Covered | Load bounds, queues; sizing in the performance page |
| A.8.7 Malware protection | Shared | Partial | Image scanning (Trivy); host AV is the bank's |
| A.8.8 Technical vulnerability management | Donka | Covered | cargo audit + Trivy in CI; `SECURITY.md` timelines |
| A.8.9 Configuration management | Donka | Covered | `donka.yaml`, secure defaults (DNK-85/86/87), doctor |
| A.8.10 Information deletion | Shared | Covered | Retention jobs; person deletion (DNK-51/75) |
| A.8.11 Data masking | Donka | Covered | PII redaction in feed, replay, explanations (DNK-54/88) |
| A.8.12 Data leakage prevention | Shared | Covered | PII redaction, no secrets in support bundle/logs, export controls by role |
| A.8.13 Information backup | Shared | Partial | Backup and restore rehearsed end to end (DNK-50); recovery values in the install guides |
| A.8.14 Redundancy of information processing facilities | Customer | Partial | Two-Runtime topology rehearsed (DNK-50) and documented; the bank runs the second Runtime |
| A.8.15 Logging | Donka | Covered | Structured logs; immutable audit; no secrets logged |
| A.8.16 Monitoring activities | Shared | Covered | Health/readiness, audit; bank SIEM consumes exports |
| A.8.17 Clock synchronisation | Customer | Customer | Host NTP; timestamps are host-provided |
| A.8.18 Privileged utility programs | Donka | Covered | `donka-admin` on the host only, not over HTTP |
| A.8.19 Installation of software on operational systems | Shared | Covered | Only the published images; signed artifacts verified |
| A.8.20 Networks security | Shared | Covered | Localhost defaults (DNK-86), host allowlists (DNK-84) |
| A.8.21 Security of network services | Shared | Covered | Same as A.8.20; TLS at the bank proxy |
| A.8.22 Segregation of networks | Customer | Customer | Deployment topology; Studio/Runtime split supports zones |
| A.8.23 Web filtering | Customer | Customer | Bank proxy policy |
| A.8.24 Cryptography | Donka | Covered | AES-256-GCM, Argon2, Ed25519; rotation (DNK-87) |
| A.8.25 Secure development life cycle | Donka | Covered | SSDF mapping ([ssdf.md](ssdf.md)) |
| A.8.26 Application security requirements | Donka | Covered | ASVS page ([asvs.md](asvs.md)) |
| A.8.27 Secure architecture | Donka | Covered | ADRs, threat model, deny-by-default |
| A.8.28 Secure coding | Donka | Covered | Memory-safe stacks, strict linters as merge gates |
| A.8.29 Security testing | Donka | Covered | CI suites; pentest DNK-69 part 2 |
| A.8.30 Outsourced development | Donka | N/A | No outsourced development; fork policy in threat model RA-1 |
| A.8.31 Separation of environments | Donka | Covered | Per-environment Runtime tokens and artifacts |
| A.8.32 Change management | Shared | Covered | Versioned releases, approvals, rollback (DNK-52/53) |
| A.8.33 Test information | Donka | Covered | Synthetic data only in tests and screenshots |
| A.8.34 Audit of information systems | Shared | Covered | Read-only audit access; support bundle without secrets |

## SOC 2 trust services criteria (software-relevant)

SOC 2 reports examine the operating company over time; Donka the company holds none
(Phase 3 programme). The rows state what the software gives a bank's own SOC 2 scope.

| Criteria | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| CC1 Organisation and management | Customer | Customer | Bank's control environment |
| CC2 Communication and information | Shared | Covered | Guides EN/FR, audit exports, support bundle |
| CC3 Risk assessment | Shared | Covered | Threat model + risk rows per story (this dossier) |
| CC4 Monitoring of controls | Shared | Partial | CI gates cover the code; operating monitoring is the bank's |
| CC5 Control activities | Shared | Covered | Deny-by-default, approvals, segregation in product |
| CC6 Logical and physical access | Shared | Covered | ASVS V2–V4; physical stays Customer |
| CC7 System operations | Shared | Covered | Doctor, health/readiness, immutable logs; incident process EX-2 (closed by DNK-50's rehearsal) |
| CC8 Change management | Shared | Covered | Versioned releases, rollback, signed artifacts |
| CC9 Risk mitigation | Shared | Partial | Vendor risk is this dossier; pentest DNK-69 part 2 pending |
| A1–A3 Availability | Shared | Partial | Backup/restore rehearsal run (DNK-50); recovery values published in the install guides |
| C1 Confidentiality | Donka | Covered | Encryption, redaction, secret files; classification EX-1 |
| PI1 Processing integrity | Donka | Covered | Contract validation, immutable results, replay (DNK-52/54) |
| P1–P8 Privacy | Shared | Covered | People-rights features (DNK-75); notices are the bank's text |

## GDPR (Regulation (EU) 2016/679)

Applies where the bank or its customers are in scope; the software is the processor-side
tooling, the bank is controller.

| Article / duty | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| Art. 5 principles (minimisation, purpose, storage limitation) | Shared | Covered | Contracts declare inputs; PAN refusal; retention defaults |
| Art. 6/9 lawful basis, special categories | Customer | Customer | Bank's legal basis; protected-characteristic analysis is DNK-71 |
| Art. 12–14 transparency | Shared | Partial | Explanations exist (DNK-88); the notice text is the bank's |
| Art. 15 access | Donka | Covered | Person search across decisions (DNK-75) |
| Art. 16/17 rectification and erasure | Donka | Covered | Person deletion with legal-hold handling (DNK-75) |
| Art. 18/20 restriction, portability | Shared | Partial | Exports exist (audit CSV, archives); a per-person export pack is DNK-75 scope |
| Art. 21/22 objection; automated decisions | Shared | Covered | Contest workflow (DNK-75); human review paths in approvals |
| Art. 25 data protection by design | Donka | Covered | Secure defaults DNK-85/86/87; deny-by-default |
| Art. 28 processor terms | Customer | Customer | Contractual; sub-processor list is the deployment's |
| Art. 30 records of processing | Shared | Partial | Audit log supports it; the register document is the bank's |
| Art. 32 security of processing | Shared | Covered | ASVS matrix; encryption and access control rows |
| Art. 33/34 breach notification | Shared | Partial | Detection inputs (logs, audit); notification duty is the bank's; recovery rehearsed (DNK-50) |
| Art. 35 DPIA | Shared | Partial | This dossier + people-rights pages are inputs; the DPIA is the bank's |
| Art. 44+ transfers | Customer | Covered | On-premise; no Donka-side transfer exists to restrict |

## ISO 22301:2019 (business continuity)

Continuity is operated by the bank; the software rows are the recovery properties.

| Clause theme | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| Business impact analysis, recovery objectives | Shared | Partial | Recovery point 24 h and recovery time 1 h published in the install guides; measured 24 s with no record lost in DNK-50's rehearsal |
| Backup and restore | Shared | Partial | Shipped and rehearsed end to end (DNK-50), including a sign-in with a second step on restored data |
| Redundancy of decision service | Shared | Partial | Two-Runtime topology rehearsed (DNK-50): decisions keep answering with one Runtime or Studio stopped |
| Incident response structure | Shared | Partial | Runbooks exist; recovery exercise run (DNK-50; ASVS EX-2 closed) |
| Exercising and testing the plan | Customer | Partial | Bank exercises its plan; Donka supplies the rehearsal script |

## EU AI Act (Regulation (EU) 2024/1689)

Creditworthiness evaluation of natural persons is high-risk (Annex III). Donka is a
rule-based decision system: no model is trained by Donka, and any language-model endpoint
is the bank's own choice (modules). Duties split deployer (bank) / provider accordingly.

| Duty | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| Risk management system (Art. 9) | Shared | Partial | Threat model + fairness analysis (DNK-71, Phase 2) complete the file |
| Data and data governance (Art. 10) | Shared | Covered | Contracts, validation, PAN refusal; training data N/A (no training) |
| Technical documentation (Art. 11) | Donka | Covered | Architecture, artifact format, this dossier |
| Record-keeping (Art. 12) | Donka | Covered | Sealed decision log, immutable audit (DNK-54) |
| Transparency to deployers (Art. 13) | Donka | Covered | Guides EN/FR, capability map, performance page |
| Human oversight (Art. 14) | Shared | Covered | Approvals, four-eyes, manual review queues; bank staffs them |
| Accuracy, robustness, cybersecurity (Art. 15) | Donka | Covered | ASVS matrix; engine validation and bounds |
| Deployer duties (Art. 26) | Customer | Customer | Bank operates, monitors, keeps its registers |
| Fundamental-rights impact (Art. 27) | Shared | Partial | Product inputs exist; the assessment is the bank's, DNK-71 supports it |
| GPAI / foundation models | — | N/A | Donka trains and ships no model; optional LLM endpoint is customer-supplied |

## Malabo Convention and regional banking rules

| Framework | Owner | Status | Evidence / note |
| --- | --- | --- | --- |
| Malabo Convention (AU, 2014; in force 8 June 2023): data-protection principles, security of information systems, cybercrime cooperation | Shared | Covered | Principles map to the GDPR rows above; ratification and national transposition are checked per pilot country before signature |
| CEMAC — COBAC (banking supervision, BEAC): outsourcing/ICT risk expectations for credit institutions | Shared | Partial | This dossier is the supplier file; the bank files its own outsourcing declaration; country annex added at first CEMAC signature |
| UEMOA — Commission Bancaire de l'UMOA / BCEAO instructions | Shared | Partial | Same supplier-file role; country annex at first UEMOA signature |
| Guinea — banking and data-protection rules | Shared | Partial | Same supplier-file role; annex at first Guinean signature |
| Cameroon Law No. 2024/017 of 23 December 2024 (personal data; full compliance by 23 June 2026) | Shared | Covered | Rights rows mirror the GDPR matrix (access, erasure, objection, automated-decision safeguards); the national authority's authorisations are the bank's filings |

Each matrix starts from the strictest common rules above and gains a country section when
a customer there signs (story decision, 2026-10-25). Legal texts are cited by their
official references: Regulation (EU) 2016/679; Regulation (EU) 2024/1689; AU Convention on
Cyber Security and Personal Data Protection (Malabo, 27 June 2014); Cameroon Law
No. 2024/017 of 23 December 2024. Counsel confirms applicability per deployment.
