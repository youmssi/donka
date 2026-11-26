-- The decision log in monthly partitions (DNK-68). Records are kept for years; one table of
-- tens of millions of rows makes the retention purge a long delete and slows searches. Each
-- month of evaluation time is its own partition: the purge drops whole expired months.
--
-- The records move as they are: same ids, same sealed payloads, nothing re-encrypted.

ALTER TABLE decision_records RENAME TO decision_records_unpartitioned;
DROP TRIGGER decision_records_append_only ON decision_records_unpartitioned;
DROP TRIGGER decision_records_no_truncate ON decision_records_unpartitioned;
-- Its indexes would keep the names the partitioned table's take; the copy below reads it once.
ALTER TABLE decision_records_unpartitioned DROP CONSTRAINT decision_records_pkey;
DROP INDEX decision_records_recent_idx, decision_records_reference_idx,
    decision_records_decision_idx, decision_records_outcome_idx,
    decision_records_retention_idx, decision_records_key_idx;

-- A partitioned table's unique keys include its partition key: a record is (id, evaluated_at).
-- The feed still stores a record sent twice once: it looks the id up before inserting.
CREATE TABLE decision_records (
    id           uuid NOT NULL,
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
    payload      bytea NOT NULL,
    PRIMARY KEY (id, evaluated_at)
) PARTITION BY RANGE (evaluated_at);

CREATE INDEX decision_records_id_idx ON decision_records (id);
CREATE INDEX decision_records_recent_idx ON decision_records (project_id, evaluated_at DESC, id);
CREATE INDEX decision_records_reference_idx ON decision_records (project_id, reference)
    WHERE reference IS NOT NULL;
CREATE INDEX decision_records_decision_idx ON decision_records (project_id, decision_key, evaluated_at DESC);
CREATE INDEX decision_records_outcome_idx ON decision_records (project_id, outcome, evaluated_at DESC);
CREATE INDEX decision_records_retention_idx ON decision_records (evaluated_at);
CREATE INDEX decision_records_key_idx ON decision_records (key_id, id);

-- Row triggers on the partitioned table apply to every partition, present and future.
CREATE TRIGGER decision_records_append_only
    BEFORE UPDATE OR DELETE ON decision_records
    FOR EACH ROW EXECUTE FUNCTION donka_decision_records_guard();
CREATE TRIGGER decision_records_no_truncate
    BEFORE TRUNCATE ON decision_records
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
REVOKE UPDATE, TRUNCATE ON decision_records FROM CURRENT_USER;
GRANT UPDATE (key_id, nonce, payload) ON decision_records TO CURRENT_USER;

-- The partition of the month holding `at` (UTC months), created if missing. Studio creates the
-- coming months ahead and the month of any record it receives; concurrent callers wait on one
-- lock so a month is created once. Each partition refuses TRUNCATE and direct updates like the
-- table itself: statement triggers and privileges are per table.
CREATE FUNCTION donka_decision_records_partition(at timestamptz) RETURNS text
LANGUAGE plpgsql AS $$
DECLARE
    month_start timestamptz := date_trunc('month', at AT TIME ZONE 'UTC') AT TIME ZONE 'UTC';
    name text := 'decision_records_' || to_char(month_start AT TIME ZONE 'UTC', 'YYYY_MM');
BEGIN
    IF to_regclass(name) IS NOT NULL THEN
        RETURN name;
    END IF;
    PERFORM pg_advisory_xact_lock(hashtext('donka_decision_records_partition'));
    IF to_regclass(name) IS NULL THEN
        EXECUTE format(
            'CREATE TABLE %I PARTITION OF decision_records FOR VALUES FROM (%L) TO (%L)',
            name, month_start, month_start + interval '1 month');
        EXECUTE format(
            'CREATE TRIGGER decision_records_no_truncate BEFORE TRUNCATE ON %I '
            'FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation()', name);
        EXECUTE format('REVOKE UPDATE, TRUNCATE ON %I FROM CURRENT_USER', name);
    END IF;
    RETURN name;
END;
$$;

-- A partition for every month that has records, then the records, as they are.
SELECT donka_decision_records_partition(month)
FROM (SELECT DISTINCT date_trunc('month', evaluated_at AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' AS month
      FROM decision_records_unpartitioned) months;
SELECT donka_decision_records_partition(now());
INSERT INTO decision_records SELECT * FROM decision_records_unpartitioned;
DROP TABLE decision_records_unpartitioned;

-- The decision log's partitions with the instant each one ends: what the purge reads.
CREATE VIEW decision_records_partitions AS
SELECT c.relname::text AS name,
       (regexp_match(pg_get_expr(c.relpartbound, c.oid), 'TO \(''([^'']+)''\)'))[1]::timestamptz AS ends_at
FROM pg_inherits i
JOIN pg_class c ON c.oid = i.inhrelid
WHERE i.inhparent = 'decision_records'::regclass;

-- The retention purge drops an expired month whole. Dropping a partition is allowed only to a
-- transaction that set `donka.purge_before` at or past the partition's end.
CREATE FUNCTION donka_drop_decision_records_partition(partition text) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
    ends timestamptz;
BEGIN
    SELECT ends_at INTO ends FROM decision_records_partitions WHERE name = partition;
    IF ends IS NULL THEN
        RAISE EXCEPTION '% is not a decision_records partition', partition;
    END IF;
    IF (ends > NULLIF(current_setting('donka.purge_before', true), '')::timestamptz) IS NOT FALSE THEN
        RAISE EXCEPTION 'partition % is not past the retention period', partition
            USING ERRCODE = 'insufficient_privilege';
    END IF;
    EXECUTE format('ALTER TABLE decision_records DETACH PARTITION %I', partition);
    EXECUTE format('DROP TABLE %I', partition);
END;
$$;
