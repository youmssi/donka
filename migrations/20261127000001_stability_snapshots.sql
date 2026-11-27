-- Population stability snapshots (DNK-57). A snapshot holds counts, shares and indices only:
-- no decrypted value and no record identifier, so it can be kept and shown without holding
-- anything personal, and it needs no purging when the records it measured age out.
CREATE TABLE stability_snapshots (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    decision_key text NOT NULL CHECK (char_length(decision_key) BETWEEN 1 AND 200),
    -- The run that produced it: an owner on demand, or the scheduled pass.
    started_by uuid REFERENCES users (id),
    scheduled boolean NOT NULL DEFAULT false,
    -- The reference period the recent one is compared with.
    reference_from timestamptz NOT NULL,
    reference_until timestamptz NOT NULL,
    recent_from timestamptz NOT NULL,
    recent_until timestamptz NOT NULL,
    -- The configuration that produced the bands, recorded so a later reader is not left
    -- guessing which thresholds a snapshot was measured against.
    stable_below double precision NOT NULL CHECK (stable_below > 0),
    shifted_at double precision NOT NULL CHECK (shifted_at >= stable_below),
    minimum_bin integer NOT NULL CHECK (minimum_bin > 0),
    records_read integer NOT NULL DEFAULT 0 CHECK (records_read >= 0),
    -- The weekly decision mix, and one shift per characteristic, as computed.
    mix jsonb NOT NULL,
    shifts jsonb NOT NULL,
    -- The worst band in `shifts`, so the alert and the list do not reparse the document.
    worst_band text NOT NULL CHECK (worst_band IN ('stable', 'moved', 'shifted')),
    created_at timestamptz NOT NULL,
    CHECK (reference_from < reference_until),
    CHECK (recent_from < recent_until)
);
CREATE INDEX stability_snapshots_project_recent_idx
    ON stability_snapshots (project_id, decision_key, created_at DESC);

-- An owner is mailed once when a measure crosses into breach, not on every snapshot, so an
-- expected drift does not become a recurring alert. A row per decision records the band the
-- last alert was sent for; a snapshot alerts only when the band worsens past it.
CREATE TABLE stability_alerts (
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    decision_key text NOT NULL CHECK (char_length(decision_key) BETWEEN 1 AND 200),
    alerted_band text NOT NULL CHECK (alerted_band IN ('stable', 'moved', 'shifted')),
    snapshot_id uuid NOT NULL REFERENCES stability_snapshots (id) ON DELETE CASCADE,
    alerted_at timestamptz NOT NULL,
    PRIMARY KEY (project_id, decision_key)
);

-- The alert outbox. Each module owns its own, as identity does with account_emails and the
-- release module with approval_emails, and the shared email worker delivers all three.
CREATE TABLE stability_alert_emails (
    id uuid PRIMARY KEY,
    snapshot_id uuid NOT NULL REFERENCES stability_snapshots (id) ON DELETE CASCADE,
    recipient text NOT NULL,
    subject text NOT NULL,
    body text NOT NULL,
    created_at timestamptz NOT NULL,
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamptz NOT NULL,
    sent_at timestamptz,
    -- Set when every attempt failed.
    abandoned_at timestamptz,
    last_error text
);
CREATE INDEX stability_alert_emails_due ON stability_alert_emails (next_attempt_at)
    WHERE sent_at IS NULL AND abandoned_at IS NULL;
