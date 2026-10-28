-- The rights of the people decisions are about (DNK-75): finding their records, giving them a
-- report, anonymising their records and recording a contest.

-- Keyed hashes (HMAC-SHA-256 under DONKA_PERSON_INDEX_KEY) of the identifiers a record holds:
-- a person is found by their national ID or phone without decrypting the log, and the table
-- holds no identifier in clear. The retention purge removes the rows of the records it removes.
CREATE TABLE decision_record_people (
    project_id   uuid NOT NULL REFERENCES projects (id),
    hash         bytea NOT NULL,
    record_id    uuid NOT NULL,
    evaluated_at timestamptz NOT NULL,
    PRIMARY KEY (project_id, hash, record_id)
);
CREATE INDEX decision_record_people_record_idx ON decision_record_people (record_id, evaluated_at);
CREATE INDEX decision_record_people_retention_idx ON decision_record_people (evaluated_at);

-- Records received before this story are indexed by `donka-app decision-log index-people`,
-- batch by batch from where it stopped; records received from now on are indexed as they come.
CREATE TABLE decision_record_people_backfill (
    only_row       boolean PRIMARY KEY DEFAULT true CHECK (only_row),
    received_before timestamptz NOT NULL,
    after_evaluated_at timestamptz,
    after_id       uuid,
    finished_at    timestamptz
);
INSERT INTO decision_record_people_backfill (received_before) VALUES (now());

-- A rule an owner documents that requires keeping records whole (a dispute, an anti-money-
-- laundering file): while it runs, the project's records cannot be anonymised.
CREATE TABLE retention_rules (
    id              uuid PRIMARY KEY,
    project_id      uuid NOT NULL REFERENCES projects (id),
    description     text NOT NULL CHECK (char_length(btrim(description)) BETWEEN 1 AND 500),
    legal_reference text CHECK (legal_reference IS NULL OR char_length(btrim(legal_reference)) BETWEEN 1 AND 200),
    until           date NOT NULL,
    created_by      uuid NOT NULL REFERENCES users (id),
    created_at      timestamptz NOT NULL,
    ended_by        uuid REFERENCES users (id),
    ended_at        timestamptz
);
CREATE INDEX retention_rules_project_idx ON retention_rules (project_id, until);

-- A person contests a decision; the review happens in the lender's systems, its outcome is
-- recorded here. Removed with the record by the retention purge.
CREATE TABLE decision_contests (
    id              uuid PRIMARY KEY,
    -- The order contests were recorded in, even within one instant.
    seq             bigint GENERATED ALWAYS AS IDENTITY,
    project_id      uuid NOT NULL REFERENCES projects (id),
    record_id       uuid NOT NULL,
    evaluated_at    timestamptz NOT NULL,
    status          text NOT NULL CHECK (status IN ('open', 'upheld', 'revised')),
    reason          text NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 1 AND 2000),
    opened_by       uuid NOT NULL REFERENCES users (id),
    opened_at       timestamptz NOT NULL,
    review          text CHECK (review IS NULL OR char_length(btrim(review)) BETWEEN 1 AND 2000),
    revised_outcome text CHECK (revised_outcome IS NULL OR char_length(btrim(revised_outcome)) BETWEEN 1 AND 200),
    reviewed_by     uuid REFERENCES users (id),
    reviewed_at     timestamptz,
    CHECK ((status = 'open') = (reviewed_at IS NULL)),
    CHECK ((status = 'revised') = (revised_outcome IS NOT NULL))
);
CREATE INDEX decision_contests_record_idx ON decision_contests (project_id, record_id);
CREATE INDEX decision_contests_retention_idx ON decision_contests (evaluated_at);

-- When the person's fields were removed from a record. Anonymising is the third change a
-- record accepts: a transaction that sets `donka.anonymise` may replace the sealed payload once,
-- setting `anonymised_at`; it cannot be undone, and nothing else may change.
ALTER TABLE decision_records ADD COLUMN anonymised_at timestamptz;

CREATE OR REPLACE FUNCTION donka_decision_records_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE'
        AND OLD.evaluated_at < NULLIF(current_setting('donka.purge_before', true), '')::timestamptz
    THEN
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE'
        AND (NEW.id, NEW.project_id, NEW.release_id, NEW.environment, NEW.decision_key,
             NEW.reference, NEW.status, NEW.outcome, NEW.evaluated_at, NEW.duration_us,
             NEW.received_at, NEW.token_id)
            IS NOT DISTINCT FROM
            (OLD.id, OLD.project_id, OLD.release_id, OLD.environment, OLD.decision_key,
             OLD.reference, OLD.status, OLD.outcome, OLD.evaluated_at, OLD.duration_us,
             OLD.received_at, OLD.token_id)
    THEN
        IF current_setting('donka.reseal', true) = 'on'
            AND NEW.key_id IS DISTINCT FROM OLD.key_id
            AND NEW.anonymised_at IS NOT DISTINCT FROM OLD.anonymised_at
        THEN
            RETURN NEW;
        END IF;
        IF current_setting('donka.anonymise', true) = 'on'
            AND OLD.anonymised_at IS NULL
            AND NEW.anonymised_at IS NOT NULL
        THEN
            RETURN NEW;
        END IF;
    END IF;
    RAISE EXCEPTION 'table % is append-only: % is not allowed', TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

GRANT UPDATE (key_id, nonce, payload, anonymised_at) ON decision_records TO CURRENT_USER;
