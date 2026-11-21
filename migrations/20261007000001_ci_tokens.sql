-- CI tokens (DNK-20): owners issue them per project so CI pipelines can pull
-- the project's release artifacts (POST /api/v1/rules-sync). Read-only: a
-- token can only resolve and download artifacts of its own project. Shown
-- once; only its SHA-256 is kept.
CREATE TABLE ci_tokens (
    id           uuid PRIMARY KEY,
    project_id   uuid NOT NULL REFERENCES projects (id),
    name         text NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 100),
    hash         text NOT NULL UNIQUE,
    -- The token's last characters, so people can tell tokens apart.
    hint         text NOT NULL,
    created_by   uuid NOT NULL REFERENCES users (id),
    created_at   timestamptz NOT NULL,
    revoked_by   uuid REFERENCES users (id),
    revoked_at   timestamptz,
    -- Last time a pipeline used it, so stale tokens can be spotted.
    last_used_at timestamptz
);

CREATE INDEX ci_tokens_project ON ci_tokens (project_id, created_at DESC);
