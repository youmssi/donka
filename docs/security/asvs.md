# ASVS level 2 self-assessment

**Standard:** OWASP Application Security Verification Standard 4.0.3, level 2.
**Assessed:** 2026-11-09, against `develop` at `dc3229d` (after DNK-87 and DNK-73 part 1).
**Status of this page:** self-assessed (DNK-73). No assessor has verified it; the Trust page
states that exactly. Failures are written exceptions below (EX-n), each with an owner; they
become stories when a customer or the roadmap requires them.

Evidence names the source a reader can check: a file, a CI job (`.github/workflows/ci.yml`
unless named), or a published page of this dossier. Status: **Met**, **Partial** (the
control exists with a named limit), **N/A** (the situation does not arise, with the reason),
**Exception** (not met; accepted in writing below).

## V1 — Architecture, design and threat modelling

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 1.1.1 | Secure SDLC, used and documented | Met | `AGENTS.md`, `CONTRIBUTING.md` (checks §5, gates before publication) |
| 1.1.2 | Threat modelling for changes | Met | [`threat-model.md`](threat-model.md), updated with DNK-83 to DNK-87 |
| 1.1.3 | Security user stories | Met | `docs/backlog/`, security allocation in stage-1 stories |
| 1.1.4 | Security control requirements defined | Met | `docs/engineering/principles.md` (owe nothing, deny by default) |
| 1.1.5 | Architecture components defined from security view | Met | `docs/adr/001`–`004`, `docs/ARCHITECTURE.md`, threat model §1 |
| 1.1.6 | Centralised, simple, vetted security controls | Met | `crates/` boundaries (`docs/engineering/backend.md`) |
| 1.1.7 | Up-to-date security guidance for developers | Met | `docs/engineering/` guides, `AGENTS.md` |
| 1.1.8 | Data classification defined | Partial | PII marking exists (`x-donka.pii`, decision-log redaction); no written classification of every data kind — EX-1 |
| 1.2.1 | Unique application identity (no shared accounts) | Met | `crates/identity`, per-person accounts only |
| 1.2.2 | Communications between components authenticated/encrypted as needed | Met | Runtime tokens (hashed, scoped), decision-log token, TLS at the bank proxy |
| 1.2.3 | High-value flows re-authenticate or step up | Met | Two-step sign-in (DNK-59), `DONKA_MFA_REQUIRED` policy |
| 1.2.4 | Externally reachable features inventoried for abuse | Met | Threat model attack surfaces AS-1..AS-17 |
| 1.4.1 | Trusted enforcement points (server-side) | Met | Deny-by-default router (`apps/app/src/lib.rs`), server-side checks in the crates |
| 1.4.2 | No security data in untrusted stores | Met | Decisions/contracts in PostgreSQL; browser holds only the session cookie |
| 1.4.3 | Segregation in system design | Met | Studio/Runtime split (ADR-001), per-tenant Runtime token (DNK-62) |
| 1.4.4 | Feature-level authorisation enforced server-side | Met | Project roles checked in `crates/project`; live permission journeys in CI |
| 1.4.5 | No reliance on client-side controls | Met | All validation repeated server-side (`crates/engine` contract checks) |
| 1.5.1 | Input/output interfaces defined | Met | OpenAPI (`apps/app` utoipa), `docs/artifact-format.md` |
| 1.5.2 | Serialisation safety | Met | `serde` typed parsing everywhere; no native serialisation |
| 1.5.3 | Output interfaces minimise data | Partial | Decision-log redaction and explanation rights (DNK-54/88); no field-level minimisation review per screen — EX-1 |
| 1.5.4 | Schema validation on shared data | Met | Contract JSON Schema (DNK-37), artifact format checks in the Runtime loader |
| 1.6.1 | Key management documented | Met | Installation guides (EN/FR), keyring rotation (decision log, artifact keys DNK-87) |
| 1.6.2 | Secrets not in source | Met | Secrets as files (`secrets/`), `.gitignore`, CI secret scanning on forks review |
| 1.6.3 | Inventory of sensitive data lifetimes | Partial | Retention documented for the decision log (5 y default) and audit; not for every store — EX-1 |
| 1.6.4 | Clients told minimum data to send | Met | Contracts declare inputs; PAN refusal (DNK-73 part 1, [`pci-dss-scope.md`](pci-dss-scope.md)) |
| 1.7.1 | Errors logged centrally, no sensitive data in logs | Met | Structured tracing; decision-log key never logged (`crates/decision-log`) |
| 1.7.2 | Security incidents process | Partial | `SECURITY.md` reporting + runbooks (DNK-50 pending rehearsal) — EX-2 |
| 1.8.1 | Sensitive data identified/protected | Met | `pii` annotations drive redaction in feed, replay and explanations |
| 1.8.2 | Data protection technical measures | Met | AES-256-GCM sealing, hashed lookup keys (`crates/decision-log`) |
| 1.9.1 | Identity of communication endpoints verified | Met | TLS at proxy; S3/MinIO endpoints configured, connector hosts allowlist (DNK-84) |
| 1.9.2 | Backend TLS settings follow guidance | Met | Deployment is the bank's proxy (install guides); Runtime HTTPS supported (`HTTP_SSL__`) |
| 1.10.1 | Code integrity (no unreviewed code to production) | Met | Branch + PR + green CI before merge (`CONTRIBUTING.md`), squash history |
| 1.11.1 | Business logic flows defined, abuse considered | Met | Threat model ID/RA/CN rows; approval chains (four-eyes, DNK-52) |
| 1.11.2 | External calls timed/bounded | Met | Connector bounds (DNK-84), load limits, evaluation timeouts in the Runtime |
| 1.11.3 | Race conditions on high-value flows considered | Met | Version/row checks, decision-log sequence, duplicate-sign-in throttles; concurrency tests |
| 1.12.1 | Safe file upload guidance | Met | Pack import validates archives (DNK-55); no general upload exists |
| 1.12.2 | Uploaded files on separate domain/untrusted handling | N/A | No user file upload is served back; pack files are validated JSON |
| 1.13.1 | API components defined | Met | OpenAPI is generated, never hand-edited (CONTRIBUTING) |
| 1.14.1 | Build environment segregated/controlled | Met | GitHub-hosted CI, locked dependencies (`--locked`, `--frozen-lockfile`) |
| 1.14.2 | Compiler flags hardening, no dead code | Met | Strict Clippy workspaces, release profiles in each `Cargo.toml` |
| 1.14.3 | Trusted CDN/subresource policy | Met | No CDN; static export same-origin, CSP hash list (DNK-85) |
| 1.14.4 | Third-party components inventoried | Met | `Cargo.lock`, `pnpm-lock.yaml`, SBOM in dossier (DNK-66 Trust page) |
| 1.14.5 | Dependency lifecycle, no unsupported components | Partial | Engine pins checked (`scripts/check-engine-version.sh`); formal end-of-life watch is Dependabot triage — EX-3 |
| 1.14.6 | Security configuration hardening guidance | Met | Secure defaults (DNK-85/86/87), install guides EN/FR |

## V2 — Authentication

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 2.1.1 | Passwords ≥ 12 characters | Met | `crates/identity` password policy + invitation/setup journeys |
| 2.1.2 | Passwords up to 64+ characters allowed | Met | Same policy (long input refused only before hashing to bound CPU, far above 64) |
| 2.1.3 | No periodic rotation forced | Met | No expiry rule exists |
| 2.1.4 | Unicode, spaces allowed; no truncation | Met | Password handled as UTF-8 string into Argon2 |
| 2.1.5 | Change requires current password | Met | Settings journeys in CI |
| 2.1.6 | Change compared like a new password | Met | Same policy path |
| 2.1.7 | Breached-password screening | Exception | Not screened against a breach corpus — EX-4 |
| 2.1.8 | Strength meter shown | Met | Setup screens (password guidance in EN/FR catalogues) |
| 2.1.9 | No composition rules beyond length/strength | Met | Policy is length/strength based |
| 2.1.10 | No forced periodic change, history kept | Met | No rotation forcing; setup links single-use |
| 2.1.11 | Paste allowed, password managers work | Met | Standard password inputs (live journeys paste/type) |
| 2.1.12 | Show while typing allowed | Met | Web app password fields |
| 2.2.1 | Anti-automation on authentication | Met | Per-address + per-account throttles, lockout (`DONKA_SIGN_IN_*`) |
| 2.2.2 | Weak authenticator guidance | N/A | Only passwords + TOTP exist |
| 2.2.3 | Notifications of auth decisions to users | Met | Invitation/reset emails, sign-in code emails (Mailpit-tested journeys) |
| 2.3.1 | System-generated secrets shown once, verified hashed | Met | Setup/reset tokens hashed at rest, single-use |
| 2.3.2 | Email/link secrets expire and single-use | Met | Setup link hours, reset minutes (config), one-use checks in journeys |
| 2.3.3 | Renewal instructions when compromised | Partial | Admin can delete people / rotate tokens; no self-service "sign out everywhere" — EX-5 |
| 2.4.1 | Passwords hashed with approved one-way function | Met | Argon2 (`crates/identity`, off-thread verify) |
| 2.4.2 | Adaptive hashing work factor | Met | Argon2 parameters in `crates/identity` |
| 2.4.3 | No plaintext recovery (reset, not retrieve) | Met | Reset flow only |
| 2.4.4 | Legacy hashes upgraded on login | N/A | No legacy scheme predates Argon2 in this codebase |
| 2.4.5 | Shared secrets hashed like passwords | Met | Runtime/CI tokens stored as SHA-256 hashes |
| 2.5.1 | Generated secrets not used for password reset alone | Met | Reset uses mailed single-use token |
| 2.5.2 | Email used only for reset/verify, not as factor alone | Met | Mail link + password; second step is TOTP |
| 2.5.3 | Generated secret ≥ 20 bits entropy, rate-limited | Met | Random tokens, throttled endpoints |
| 2.5.4 | Shared secrets not modifiable by untrusted parties | Met | Token administration is owner-only, audited |
| 2.5.5 | Forgot-password does not lock account | Met | Reset flow independent of lockout |
| 2.5.6 | Forgotten/generate endpoints equivalent protections | Met | Same throttle middleware |
| 2.5.7 | Account creation + password change same strength | Met | One policy in `crates/identity` |
| 2.6.1–2.6.3 | Look-up secrets (recovery codes) | N/A | No recovery codes; enrolment is admin-assisted |
| 2.7.1–2.7.6 | Out-of-band secrets | Partial | Sign-in code by email exists (single-use, expiring); voice/SMS do not — not applicable channels |
| 2.8.1–2.8.7 | One-time verifiers (TOTP) | Met | DNK-59: TOTP, sealed secrets, rate-limited, policy-required cohorts |
| 2.9.1–2.9.3 | Cryptographic authenticators | N/A | No WebAuthn yet (Phase 2 candidate; not claimed) |
| 2.10.1 | Introspection of service credentials secured | Met | Tokens hashed, scoped, revocable; introspection is internal |
| 2.10.2 | API keys have defined policy | Met | Runtime tokens (environment-scoped), CI tokens (DNK-55), expiry |
| 2.10.3 | API keys protected in transit/at rest | Met | TLS at proxy; hashes at rest; shown once |
| 2.10.4 | Service auth follows least privilege | Met | Per-environment Runtime tokens, per-secret connector hosts (DNK-84) |

## V3 — Session management

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 3.1.1 | No session tokens in URLs | Met | Cookie-only sessions; setup tokens are not sessions |
| 3.2.1 | New session at auth, regenerate on privilege change | Met | `crates/identity` session issuance on sign-in |
| 3.2.2 | Session tokens ≥ 64 bits from approved generator | Met | Random session ids (getrandom) |
| 3.2.3 | Tokens stored hashed server-side | Met | Session records keyed by hash |
| 3.2.4 | Tokens from a CSPRNG | Met | Same generator |
| 3.3.1 | Logout invalidates server-side | Met | Sign-out journey revokes |
| 3.3.2 | Idle timeout | Met | `DONKA_SESSION_IDLE_MINUTES` (480) |
| 3.3.3 | Absolute lifetime with re-authentication | Partial | Idle expiry exists; no absolute cap forcing re-login — EX-5 |
| 3.3.4 | Sessions invalidated on password change | Met | Password change revokes sessions (identity tests) |
| 3.4.1 | Cookie `Secure` | Met | `DONKA_COOKIE_SECURE=true` default |
| 3.4.2 | Cookie `HttpOnly` | Met | Session cookie flags in `apps/app/src/auth.rs` |
| 3.4.3 | Cookie `SameSite` | Met | Same-site session cookie (auth.rs) |
| 3.4.4 | Cookie path as restrictive as possible | Partial | Cookie scoped to the app origin path; API and pages share one origin — accepted, EX-6 |
| 3.4.5 | No persistent "remember me" by default | Met | No persistent cookie exists |
| 3.5.1 | Non-session tokens static-secret safe | Met | Setup/CI tokens hashed, expiring |
| 3.5.2 | JWTs verified with approved algorithms | N/A | No JWTs are issued or accepted |
| 3.5.3 | JWT lifetime/audience checks | N/A | No JWTs |
| 3.6.1 | High-risk flows re-authenticate / step up | Met | DNK-59 second step for production decisions |
| 3.7.1 | Sensitive operations need fresh authentication | Partial | Production approval needs owner + second step by policy; not a fresh password prompt — EX-5 |

## V4 — Access control

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 4.1.1 | Enforced on trusted server layer | Met | All checks in the crates; web sends no authority |
| 4.1.2 | Enforcement on every request, server-side | Met | Deny-by-default router; session middleware |
| 4.1.3 | Least privilege per role | Met | Instance admin vs project owner/editor/viewer (DNK-51) |
| 4.1.4 | Attribute/field-level where needed | Met | Owner-only audit/settings; redaction by role (DNK-54) |
| 4.1.5 | Access control fails securely | Met | Deny by default; locked forms until checks pass (DNK-48) |
| 4.2.1 | Sensitive data access protected per-object | Met | Project-scoped queries; cross-project tests (`docs/engineering/backend.md` §9) |
| 4.2.2 | Mass assignment / parameter tampering resisted | Met | Typed request bodies; server-set ownership fields |
| 4.3.1 | Directory browsing off, metadata not exposed | Met | Static export only; no VCS files in images (`.dockerignore`) |
| 4.3.2 | Admin interfaces authenticated/authorized | Met | `donka-admin` runs on the host/container, not over HTTP |
| 4.3.3 | Privilege separation (admin vs user) | Met | Instance administrators vs project roles; no shared account |

## V5 — Validation, sanitisation and encoding

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 5.1.1–5.1.3 | Allowlist input validation incl. non-HTTP input | Met | Typed JSON + contract schema; engine validates inputs (DNK-52/88) |
| 5.1.4 | Structured data validated by strong types | Met | `serde` typed models, JSON Schema (draft-07) |
| 5.1.5 | Redirects/forwards validated | Met | No open-redirect endpoints; setup links are internal paths |
| 5.2.1–5.2.2 | HTML sanitisation / no untrusted HTML execution | Met | React rendering only; no raw HTML injection points (CSP, DNK-85) |
| 5.3.1–5.3.10 | Output encoding by context (HTML, JS, URL, SQL, OS, XXE, template) | Met | React escaping; parameterised SQL (`sqlx`); no shell/SQL string building; XML not parsed |
| 5.4.1 | Memory-safe parsing bounds | Met | Body limits (16 MiB), connector response bounds (DNK-84) |
| 5.4.2 | Integer handling | Met | Typed numeric checks in engine/expressions |
| 5.4.3 | Deserialisation of untrusted data hardened | Met | `serde_json` only, size-bounded |
| 5.5.1–5.5.4 | Safe deserialisation/memory techniques | Met | Rust (memory-safe) for all server code |

## V6 — Stored cryptography

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 6.1.1–6.1.3 | Regulated data classified; crypto by policy; keys managed | Partial | Decision-log + artifact keys managed with rotation; written crypto policy is this dossier — EX-1 |
| 6.2.1–6.2.8 | Approved algorithms, no broken modes, sufficient strength | Met | AES-256-GCM, Argon2, Ed25519, SHA-256; TLS 1.2+ at proxy |
| 6.3.1–6.3.3 | Random values from approved CSPRNG | Met | `getrandom` for keys/tokens/seeds |
| 6.4.1–6.4.2 | Key material protected; secrets in a vault | Met | Secret files 0600-class handling, `_FILE` settings, never logged |

## V7 — Error handling and logging

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 7.1.1–7.1.4 | No sensitive data in errors/logs; log injection safe | Partial | API errors are coded shapes (no stack traces); tracing is structured — formal log-content review pending pentest (DNK-69 part 2) |
| 7.2.1–7.2.2 | Auth decisions logged | Met | Audit log (immutable, DNK-54) records sign-ins and decisions |
| 7.3.1–7.3.3 | Logs protected, forwarded, not writable by users | Met | Append-only audit tables; support bundle is read-only export |
| 7.4.1–7.4.3 | Generic client errors, detailed server logs, no debug in prod | Met | `ApiError` catalogue; debug endpoints absent |

## V8 — Data protection

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 8.1.1–8.1.6 | Server-side enforcement incl. per-user data limits | Met | Project scoping; pagination on feed/exports (DNK-54) |
| 8.2.1–8.2.3 | Browser caching disabled for sensitive data | Partial | API is not cached by the SPA; no explicit `Cache-Control` policy on API responses — EX-7 |
| 8.3.1–8.3.8 | Sensitive data minimal, encrypted, deletable, not in GET | Met | PII redaction, retention jobs, person deletion (DNK-51), no secrets in URLs |

## V9 — Communications

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 9.1.1–9.1.3 | TLS everywhere, strong settings, valid certificates | Met (deployment) | Bank TLS proxy (install guides); HSTS (DNK-85); Runtime HTTPS option |
| 9.2.1–9.2.5 | Server comms: TLS, SNI, cert validation, no cleartext fallback | Met | Connector HTTPS rules (DNK-84); S3/MinIO TLS endpoints configured |

## V10 — Malicious code

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 10.1.1 | Code integrity: signed/reviewed sources | Met | Protected branches, squash history, signed artifacts (DNK-87) |
| 10.2.1–10.2.6 | Malicious activity addressed (no backdoors, time bombs, Easter eggs) | Met | Small reviewed codebase; no hidden triggers |
| 10.3.1–10.3.3 | Dependency integrity, no malicious packages, updates applied | Partial | Locked deps + audits in CI (DNK-49); Scorecard pending (DNK-73 part 4) |

## V11–V14 — Business logic, files, API, configuration

| Req | Requirement (short) | Status | Evidence |
| --- | --- | --- | --- |
| 11.1.1–11.1.8 | Business logic abuse, step order, limits, anti-automation | Met | Approval chains, immutable versions, rate limits, replay protection in engine |
| 12.1.1–12.1.3 | File upload type/size, no execution | Met | Pack import: validated JSON archives, size bounds (DNK-55) |
| 12.2.1, 12.3.x, 12.4.x, 12.5.x, 12.6.1 | File display/download, path traversal, SSRF | Met | No file serving of uploads; connector SSRF allowlist + no redirects (DNK-84) |
| 13.1.1–13.1.5 | API auth on every request, content types, no over-disclosure | Met | Session/token on every route; generated OpenAPI |
| 13.2.x, 13.3.x, 13.4.x | REST/JSON, SOAP, GraphQL, WebSocket hardening | Partial | REST/JSON met; SOAP/GraphQL N/A; decision-log feed is HTTPS POST, not WebSocket |
| 14.1.1–14.1.5 | Build/deploy hardening, seg. duties | Met | Pinned CI actions policy (part 4), image scanning (Trivy in CI) |
| 14.2.1–14.2.6 | Dependency management, no unused features/debug | Met | Locked features; offline bundle job proves minimal surface |
| 14.3.1–14.3.5 | Unintended information disclosure; security headers | Met | DNK-85 headers + CSP guard; error catalogue |
| 14.4.1–14.4.7 | HTTP security headers (CSP, HSTS, framing, caching…) | Met | DNK-85 (CSP/HSTS); caching item is EX-7 |
| 14.5.1–14.5.4 | Request/header validation, CORS | Met | Same-origin app; no permissive CORS |

## Written exceptions (accepted, with owners)

| ID | Requirement | Exception and reason | Owner / next step |
| --- | --- | --- | --- |
| EX-1 | 1.1.8, 1.5.3, 1.6.1/6.1 | No single written data-classification/cryptography policy document; controls exist per feature. Accepted for the pilot; one policy page is a Phase-2 story candidate. | Product owner |
| EX-2 | 1.7.2 | Incident rehearsal not yet run (backup/restore/upgrade rehearsal is DNK-50). | DNK-50 |
| EX-3 | 1.14.5 | Dependency end-of-life watched by triage, not a scheduled report. | Maintainer, monthly triage |
| EX-4 | 2.1.7 | Passwords not screened against breach corpora (offline deployment must not call a breach service). Accepted; length+MFA policy compensates. | Product owner |
| EX-5 | 2.3.3, 3.3.3, 3.7.1 | No absolute session cap or fresh-password prompt for sensitive actions; idle timeout + second step by policy instead. Revisit if a customer requires it. | Product owner |
| EX-6 | 3.4.4 | Session cookie path covers the whole origin (pages + API share it by design). | Accepted |
| EX-7 | 8.2, 14.4 | No explicit `Cache-Control: no-store` on API responses yet; the SPA never caches API data. Becomes a small fix story if the pentest (DNK-69 part 2) raises it. | DNK-69 part 2 input |

Every other L2 requirement above is claimed **Met** or **N/A** only where the named evidence
can be opened and checked. When a change alters a control, this page changes in the same
commit (DNK-73 decision).
