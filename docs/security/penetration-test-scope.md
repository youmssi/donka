# Penetration test: scope for quotation

Donka asks an independent security firm to test the installation a bank pilot will run, before
that pilot reaches production (DNK-69). This page is what the firm quotes from. Testers must
hold a recognised certification (CREST, OSCP/OSWE or equivalent) and work under a signed
engagement letter.

## What Donka is

Donka is a self-hosted decision platform for banks and lenders:
- **Studio** (Rust/Axum API and a static Next.js web app): teams write rules, approve and
  release them.
- **Runtime** (Rust): the bank's systems call it for decisions, and it sends each decision back
  to Studio.
- **CLI** (Node.js): pulls released rules into CI/CD pipelines.

One installation per customer, run with Docker Compose from signed images. The design and its
known risks are in the [threat model](threat-model.md); the testers receive it at kick-off.

## Targets

| Target | Interfaces | Notes |
| --- | --- | --- |
| Studio web app | Browser, all screens | English and French |
| Studio API | `/api/v1`, including sign-in, second step, projects, decisions, simulate, releases, approvals, tokens, decision-log search/replay/explain, people's rights, audit export, installation and settings | OpenAPI document at `/api/v1/openapi.json` |
| Decision-log feed and rules sync | `POST /api/v1/decision-log/records`, `POST /api/v1/rules-sync` and artifact downloads | Machine-to-machine, token-authenticated |
| Runtime | Evaluate endpoints, per-project OpenAPI, health | Token per environment, rate limited |
| Rule execution | Expressions, decision tables, JavaScript function nodes, connector nodes | What a project editor can make the servers do |
| CLI | `donka pull`, `donka form`, `donka mcp` (local bridge to the editor) | Run against the test installation |
| Server tooling | `donka-admin init`, `doctor`, `upgrade`, `support-bundle`; the generated `docker-compose.yml`, `secrets/` and images | Configuration review, not host hardening |

## Environment

Donka provides a dedicated test server:
- Installed from the published images with `donka-admin init`, with the demo profile (a fake
  credit bureau) and both staging and production environments.
- Served over HTTPS behind a reverse proxy, as a bank would run it.
- No production data: the starter packs and synthetic applicants only.

Accounts provided:

| Account | Purpose |
| --- | --- |
| Administrator (with second step) | Installation settings, people, modules |
| Project owner, editor and viewer, in two projects | Authorisation boundaries between roles and projects |
| A second owner | Production approvals (four eyes) |
| A CI token, staging and production Runtime tokens, a decision-log feed token | Machine interfaces |
| Shell access to the test server (read-only user) | Configuration review |

The source code of the three repositories is available to the testers on request
(grey-box testing).

## Approach

Authenticated grey-box testing against OWASP ASVS 5.0 level 2 and the OWASP Web Security Testing
Guide, covering at least:

1. **Authentication and sessions:** lockout, rate limits, second step and recovery codes,
   invitation and reset links, session fixation and expiry, CSRF.
2. **Authorisation:** every API route across roles and projects; approval rules (an author
   approving their own release); token scopes (CI, Runtime, feed).
3. **Rule execution as an attack path:** what a project editor can make Studio's simulator and
   the Runtime do (network access, file access, resource exhaustion, data from other projects).
4. **Connectors and secrets:** where a connector may send a secret; redirects; mutual TLS and
   OAuth2 handling.
5. **Decision log and personal data:** encryption at rest, search and replay authorisation,
   redaction before explanations, people's access and anonymisation requests.
6. **Injection:** SQL, JSON Schema, expression language, header, and log injection.
7. **Release integrity:** tampering with artifacts between Studio and the Runtime.
8. **Web client:** XSS (including through rule names, decision records and imported packs),
   clickjacking, security headers.
9. **Installation:** secrets on disk, exposed ports, container privileges, the support bundle,
   image signature verification during upgrade.

The open items in the threat model are fixed before the test starts. The testers confirm that
each fix holds.

## Out of scope

- The bank's own infrastructure, network and identity provider.
- Volumetric denial of service.
- Social engineering and physical access.
- Third-party services: the bureau, the language model provider, the mail server.
- The image registry's own platform.

## Rules of engagement

- Testing window and source addresses agreed in writing.
- Donka's contact is reachable throughout the window.
- A critical finding is reported at once, without waiting for the report.
- No data leaves the test server except as evidence in the report.
- Testers destroy their copies of evidence after the retest.

## Deliverables

1. A report:
   - an executive summary;
   - each finding with its CVSS 4.0 score, evidence, reproduction steps and a recommended fix;
   - the coverage achieved against the approach above.
2. A retest of every high and critical finding once fixed, with an updated report.
3. A short attestation letter that Donka can publish on its Trust page (DNK-73): the dates, the
   scope, the method, and the counts of findings by severity and status. No vulnerability
   details.

## Effort estimate for quotation

Donka's estimate, for the firm to confirm or correct:
- Studio web and API: 4 to 5 days.
- Runtime, feed and rules sync: 1.5 days.
- Rule execution and connectors: 1.5 days.
- CLI and server tooling: 1 day.
- Reporting: 1 day.

That is **9 to 10 tester-days**, plus **2 days** for the retest. Please quote both separately,
with the earliest start date and the report delay.
