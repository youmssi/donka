-- Studio users, their sessions and one-time password-setup links (DNK-4).
-- Session and setup tokens are stored as SHA-256 hashes: a database dump
-- does not give access to live sessions.

CREATE TABLE users (
    id                       uuid PRIMARY KEY,
    email                    text NOT NULL,
    -- argon2id PHC string; NULL until the user sets a password.
    password_hash            text,
    is_admin                 boolean NOT NULL DEFAULT false,
    failed_sign_ins          integer NOT NULL DEFAULT 0 CHECK (failed_sign_ins >= 0),
    failed_window_started_at timestamptz,
    locked_until             timestamptz,
    created_at               timestamptz NOT NULL,
    CONSTRAINT users_email_is_lowercase CHECK (email = lower(email))
);
CREATE UNIQUE INDEX users_email_key ON users (email);

CREATE TABLE sessions (
    token_hash   bytea PRIMARY KEY,
    user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);

CREATE TABLE password_setup_tokens (
    token_hash bytea PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);
CREATE INDEX password_setup_tokens_user_id_idx ON password_setup_tokens (user_id);
