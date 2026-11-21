-- Production approvals (DNK-15).
--
-- Production is published only through an approval: an editor asks for the
-- release live on staging to go to production, and one of the project's
-- owners, never the release's creator nor the person asking, approves or
-- rejects it. At most one request per project waits at a time.

CREATE TABLE approvals (
    id            uuid PRIMARY KEY,
    -- The order requests were made, even within the same instant.
    seq           bigint GENERATED ALWAYS AS IDENTITY,
    project_id    uuid NOT NULL REFERENCES projects (id),
    release_id    uuid NOT NULL REFERENCES releases (id),
    requested_by  uuid NOT NULL REFERENCES users (id),
    requested_at  timestamptz NOT NULL,
    status        text NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'withdrawn')),
    decided_by    uuid REFERENCES users (id),
    decided_at    timestamptz,
    -- Why it was rejected; required for a rejection, absent otherwise.
    reason        text CHECK (reason IS NULL OR char_length(btrim(reason)) BETWEEN 1 AND 1000),
    -- The production deployment an approval queued.
    deployment_id uuid REFERENCES deployments (id),
    CHECK ((status = 'pending') = (decided_at IS NULL)),
    CHECK ((status = 'rejected') = (reason IS NOT NULL)),
    CHECK ((status = 'approved') = (deployment_id IS NOT NULL))
);
CREATE UNIQUE INDEX approvals_one_pending ON approvals (project_id) WHERE status = 'pending';
CREATE INDEX approvals_project_idx ON approvals (project_id, seq DESC);

-- Emails telling owners that a request waits for them, sent after commit. They
-- hold no secret, so they are rendered when queued, in the recipient's language.
CREATE TABLE approval_emails (
    id              uuid PRIMARY KEY,
    approval_id     uuid NOT NULL REFERENCES approvals (id),
    recipient       text NOT NULL,
    subject         text NOT NULL,
    body            text NOT NULL,
    created_at      timestamptz NOT NULL,
    attempts        integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamptz NOT NULL,
    sent_at         timestamptz,
    -- Set when every attempt failed.
    abandoned_at    timestamptz,
    last_error      text
);
CREATE INDEX approval_emails_due ON approval_emails (next_attempt_at)
    WHERE sent_at IS NULL AND abandoned_at IS NULL;
