-- Two-step sign-in with an authenticator app (DNK-59).

-- The time-based code secret of a user, sealed with DONKA_SECOND_STEP_KEY
-- (AES-256-GCM, the user id as associated data). A row without confirmed_at
-- is an enrolment in progress: it signs nobody in.
CREATE TABLE second_steps (
    user_id        uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    key_id         text NOT NULL,
    nonce          bytea NOT NULL,
    ciphertext     bytea NOT NULL,
    created_at     timestamptz NOT NULL,
    confirmed_at   timestamptz,
    -- The newest 30-second step whose code was accepted: a code works once.
    last_used_step bigint
);

-- Single-use recovery codes, stored as SHA-256 of the user id and the code.
CREATE TABLE recovery_codes (
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_hash  bytea NOT NULL,
    created_at timestamptz NOT NULL,
    used_at    timestamptz,
    PRIMARY KEY (user_id, code_hash)
);

-- Between a correct password and the second step. The token is held in a
-- short-lived cookie and stored hashed; it opens no session by itself.
CREATE TABLE pending_sign_ins (
    token_hash bytea PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- 'code': the account has a second step; 'enrol': policy requires one.
    purpose    text NOT NULL CHECK (purpose IN ('code', 'enrol')),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);
CREATE INDEX pending_sign_ins_user_id_idx ON pending_sign_ins (user_id);

-- Notices sent when a second step is turned on, off, reset or a recovery code is used.
ALTER TABLE account_emails DROP CONSTRAINT account_emails_kind_check;
ALTER TABLE account_emails ADD CONSTRAINT account_emails_kind_check CHECK (kind IN (
    'invitation', 'password_reset',
    'second_step_enabled', 'second_step_removed', 'second_step_reset', 'recovery_code_used'
));
