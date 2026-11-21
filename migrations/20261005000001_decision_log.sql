-- Decision log (DNK-18): every decision the Runtime makes, sent by the Runtime,
-- stored encrypted, searchable and replayable.
--
-- A Runtime sends records with a decision-log token that an administrator
-- issued for one environment; only its SHA-256 is kept and it is shown once.
-- A token serves every project of its environment, as one Runtime does.

CREATE TABLE decision_log_tokens (
    id          uuid PRIMARY KEY,
    environment text NOT NULL CHECK (environment IN ('staging', 'production')),
    name        text NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 100),
    hash        text NOT NULL UNIQUE,
    -- The token's last characters, so people can tell tokens apart.
    hint        text NOT NULL,
    created_by  uuid NOT NULL REFERENCES users (id),
    created_at  timestamptz NOT NULL,
    revoked_by  uuid REFERENCES users (id),
    revoked_at  timestamptz
);

-- Per project: the output field whose value is a record's outcome
-- (`decision`, `result.band`), read when the record arrives.
CREATE TABLE decision_log_settings (
    project_id    uuid PRIMARY KEY REFERENCES projects (id),
    outcome_field text CHECK (
        outcome_field IS NULL
        OR (char_length(outcome_field) BETWEEN 1 AND 200
            AND outcome_field ~ '^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$')
    ),
    updated_by    uuid NOT NULL REFERENCES users (id),
    updated_at    timestamptz NOT NULL
);

-- One evaluation. What can be searched is stored as is; what the decision
-- read and answered (input, output, error, trace) is encrypted with
-- AES-256-GCM under DONKA_DECISION_LOG_KEY (`key_id` names which key), with
-- the record id as associated data so a payload cannot be moved to another row.
CREATE TABLE decision_records (
    -- The id the Runtime gave the record; a record sent twice is stored once.
    id           uuid PRIMARY KEY,
    project_id   uuid NOT NULL REFERENCES projects (id),
    release_id   uuid NOT NULL REFERENCES releases (id),
    environment  text NOT NULL CHECK (environment IN ('staging', 'production')),
    decision_key text NOT NULL CHECK (char_length(decision_key) BETWEEN 1 AND 200),
    reference    text CHECK (reference IS NULL OR char_length(reference) BETWEEN 1 AND 200),
    status       text NOT NULL CHECK (status IN ('succeeded', 'failed')),
    outcome      text CHECK (outcome IS NULL OR char_length(outcome) BETWEEN 1 AND 200),
    evaluated_at timestamptz NOT NULL,
    duration_us  bigint NOT NULL CHECK (duration_us >= 0),
    received_at  timestamptz NOT NULL,
    token_id     uuid NOT NULL REFERENCES decision_log_tokens (id),
    key_id       text NOT NULL,
    nonce        bytea NOT NULL,
    payload      bytea NOT NULL
);
CREATE INDEX decision_records_recent_idx ON decision_records (project_id, evaluated_at DESC, id);
CREATE INDEX decision_records_reference_idx ON decision_records (project_id, reference)
    WHERE reference IS NOT NULL;
CREATE INDEX decision_records_decision_idx ON decision_records (project_id, decision_key, evaluated_at DESC);
CREATE INDEX decision_records_outcome_idx ON decision_records (project_id, outcome, evaluated_at DESC);
CREATE INDEX decision_records_retention_idx ON decision_records (evaluated_at);

-- Records never change. The only deletion allowed is the retention purge: it
-- sets `donka.purge_before` for its own transaction, and only records
-- evaluated before that moment may go.
CREATE FUNCTION donka_decision_records_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE'
        AND OLD.evaluated_at < NULLIF(current_setting('donka.purge_before', true), '')::timestamptz
    THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'table % is append-only: % is not allowed', TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

CREATE TRIGGER decision_records_append_only
    BEFORE UPDATE OR DELETE ON decision_records
    FOR EACH ROW EXECUTE FUNCTION donka_decision_records_guard();
CREATE TRIGGER decision_records_no_truncate
    BEFORE TRUNCATE ON decision_records
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
REVOKE UPDATE, TRUNCATE ON decision_records FROM CURRENT_USER;
