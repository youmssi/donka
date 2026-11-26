# Pilot integration questionnaire

[Français](pilot-questionnaire.fr.md)

To fill in with the institution's teams (IT, security, credit risk, operations) before the
installation. Each answer opens or closes a specific piece of work; the "Why" column says which.
"I don't know" is a useful answer: it becomes a question to follow up.

## 1. Pilot scope

| Question | Why |
|---|---|
| Which credit product (or other decision) does the pilot cover? Monthly volume of applications? | Starting pack, sizing |
| Who writes the rules, who approves them, who runs them? How many people? | Roles and accounts in Studio |
| Which channels ask for a decision (branch, mobile, web, back office)? Expected response time? | Runtime calls, latency targets |
| Which criteria will say the pilot succeeded? | Phase 1 exit criteria |

## 2. Hosting and network

| Question | Why |
|---|---|
| Available servers: count, operating system, CPU, memory, disk? Is Docker allowed? | Installation (DNK-64) |
| Will Studio and the production Runtime share a machine, or sit in different network zones? | Release storage: disk volume or S3 |
| Is there already an S3 store (Ceph, NetApp, Dell ECS, MinIO, cloud)? | The bank's S3 rather than a new service |
| Do the servers reach the internet? Is there an internal image registry or mirror? | Online images or offline bundle |
| Outgoing proxy, reverse proxy, internal TLS certificates? | Exposing Studio and the Runtime |
| PostgreSQL provided by the bank, or Donka's container? | Database backup and operation |

## 3. Identity and access

| Question | Why |
|---|---|
| Identity provider in use (Entra ID, AD FS, Keycloak, Okta, other)? Does it support OpenID Connect? | Single sign-on (DNK-60) |
| Required two-factor policy? | TOTP policy (DNK-59) |
| Which directory groups map to Donka administrators? | Group-to-role mapping |

## 4. Core banking system

| Question | Why |
|---|---|
| Product and version? | Preparing the integration |
| How does it expose data: REST API, SOAP, ESB or gateway, files? | Direct call, or through the bank's integration layer |
| Authentication: OAuth2 (which flow, token URL, scopes), mutual TLS (which certificate authority), API key? | Connectors (DNK-63) |
| Is a test environment reachable from the Donka server, with test data? | Testing before production |
| Which data does the decision need: outstanding balance, arrears, tenure, salary credits…? Under which names? | Input contract and field mapping |
| Call limits and guaranteed response times? | Connector timeouts, retries and circuit breaker |

## 5. Credit bureaus, KYC and AML

| Question | Why |
|---|---|
| Which external services to call (bureau, KYC, sanctions lists)? API documentation? | Further connectors |
| Contracts, test environments and credentials available? | Integration plan |

## 6. Credit policy and data

| Question | Why |
|---|---|
| Is there a current scorecard or grid? In what form (document, spreadsheet, tool)? | Import into Studio |
| Current cut-offs, limits, exceptions and approval circuit? | Rules and approvals |
| Loan history with known repayment (90 days past due)? How many loans and defaults, over which period? | Feasibility of a statistical scorecard (DNK-61) and performance monitoring |

## 7. Personal data, security and compliance

| Question | Why |
|---|---|
| Country, banking supervisor (COBAC, Commission Bancaire de l'UMOA, BCRG…) and data-protection authority? | Compliance matrices (DNK-73), rights of individuals (DNK-75) |
| Internal and regulatory rules that apply (data protection, banking supervisor)? | Encryption, redaction, retention |
| Required retention of decisions? | `DONKA_DECISION_LOG_RETENTION_DAYS` |
| Where must encryption keys be kept (vault, HSM, files)? | Secrets (DNK-65) |
| Are plain-language explanations wanted? With which model (hosted, internal)? | Language model settings (DNK-19) |
| Security questionnaire or architecture review to plan for? | Security section of the documentation (DNK-66) |

## 8. Operations

| Question | Why |
|---|---|
| Change process (committee, maintenance windows)? | Assisted upgrades (DNK-64) |
| Backup and restore policy? | DNK-50 rehearsal |
| Monitoring and logging tools (OpenTelemetry, Prometheus, other)? | Exporting metrics and traces |
| Mail server for invitations and alerts? | `DONKA_SMTP_URL` |
| Technical and security contacts at the institution? | Pilot follow-up |
