-- Invitations, password reset and the outbox of account emails (DNK-5).

-- Language of the emails a user receives (and, later, of the web app).
ALTER TABLE users
    ADD COLUMN locale text NOT NULL DEFAULT 'en' CHECK (locale IN ('en', 'fr'));

-- Emails to send after the transaction that asked for them commits. A row
-- holds no secret: the one-time link is issued at send time and only its hash
-- is stored (password_setup_tokens), so the outbox is safe in a dump.
CREATE TABLE account_emails (
    id              uuid PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind            text NOT NULL CHECK (kind IN ('invitation', 'password_reset')),
    created_at      timestamptz NOT NULL,
    attempts        integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamptz NOT NULL,
    sent_at         timestamptz,
    -- Set when every attempt failed; the person can ask again.
    abandoned_at    timestamptz,
    last_error      text
);
-- At most one pending email of each kind per user: repeated reset requests do
-- not flood an inbox.
CREATE UNIQUE INDEX account_emails_one_pending
    ON account_emails (user_id, kind) WHERE sent_at IS NULL AND abandoned_at IS NULL;
CREATE INDEX account_emails_due
    ON account_emails (next_attempt_at) WHERE sent_at IS NULL AND abandoned_at IS NULL;
