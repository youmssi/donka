# ADR-007: Sessions implemented on Studio's own PostgreSQL pool

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-4

## Context

DNK-4 needs server-side sessions in PostgreSQL. The usual crate, `tower-sessions-sqlx-store`
0.15, depends on sqlx 0.8 while Studio uses sqlx 0.9: using it compiles two sqlx versions and
needs a second, separate connection pool. It also stores the session id itself as the key, so a
database dump would contain live session ids.

## Options considered

1. **tower-sessions + sqlx store.** Familiar API. Two sqlx versions, two pools, plain ids at rest.
2. **Downgrade Studio to sqlx 0.8.** One pool, but pins the whole workspace to an older major.
3. **A small session layer in `crates/identity`** on the existing pool: a random 256-bit token in
   an httpOnly cookie, only its SHA-256 hash stored, idle expiry and server-side sign-out.

## Decision

Option 3. It is about 150 lines, uses one pool and one sqlx, and a database leak does not expose
live sessions (tokens are hashed at rest, as the playbook requires for tokens).

## Consequences

- Session behaviour (idle timeout, touch interval, sign-out, "new password ends sessions") is ours
  to maintain and is covered by integration tests.
- CSRF is handled by requiring the `x-donka-csrf` header on state-changing requests; together with
  `SameSite=Lax` and same-origin serving (ADR-002) no CSRF token store is needed.
- Every sign-in failure (wrong password, unknown email, account without password, locked account)
  returns the same `INVALID_CREDENTIALS` answer after the same argon2 work, so neither the message
  nor the timing reveals whether an account exists or is locked.
- Revisit if Studio needs features such as session listing per device or remote revocation; the
  `sessions` table already supports them.
