-- OIDC identities are scoped to an issuer and its immutable subject, with one external
-- identity per invited Studio user.
CREATE TABLE oidc_identities (
    issuer text NOT NULL CHECK (length(issuer) BETWEEN 1 AND 2048),
    subject text NOT NULL CHECK (length(subject) BETWEEN 1 AND 512),
    user_id uuid NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,
    linked_at timestamptz NOT NULL,
    PRIMARY KEY (issuer, subject)
);

-- Browser-bound OIDC states are single use and expire quickly. Only a hash is stored.
CREATE TABLE oidc_login_states (
    state_hash bytea PRIMARY KEY,
    expires_at timestamptz NOT NULL
);
CREATE INDEX oidc_login_states_expiry_idx ON oidc_login_states (expires_at);