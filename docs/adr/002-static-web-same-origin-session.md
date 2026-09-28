# ADR-002: Studio web as a static export, served by the app on the same origin

- **Status:** accepted
- **Date:** 2026-09-28
- **Story:** DNK-1

## Context

Studio is a signed-in work tool installed at each customer. The engineering playbook prefers
Server Components and a BFF so browsers never hold tokens. Every extra runtime is one more thing
each customer must operate and patch.

## Options considered

1. **Next.js server with Server Components + BFF.** Follows the playbook literally. Adds a Node.js
   container to every installation.
2. **Next.js static export served by `apps/app`** on the same origin, with an httpOnly session
   cookie set by `apps/app`.

## Decision

Option 2. The security goal of the BFF rule (no token in the browser) is met by the same-origin
httpOnly cookie. Studio is one image.

## Consequences

- No server components, server actions or route handlers in `apps/web`; pages are client-rendered
  after a static shell.
- Runtime dynamic route segments are not available: records use search params
  (`/en/projects?id=…`); locales (`en`, `fr`) are pre-generated.
- CSRF protection is required on state-changing requests (header token checked by `apps/app`).
- Revisit if Studio ever needs server rendering (public pages, SEO) — none are planned.
