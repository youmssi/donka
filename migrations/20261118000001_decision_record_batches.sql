-- Group records created by one Runtime batch evaluation while keeping each
-- evaluation independently addressable and immutable.
ALTER TABLE decision_records ADD COLUMN batch_id uuid;

CREATE INDEX decision_records_batch_idx ON decision_records (project_id, batch_id, evaluated_at DESC)
    WHERE batch_id IS NOT NULL;

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
             NEW.reference, NEW.batch_id, NEW.status, NEW.outcome, NEW.evaluated_at, NEW.duration_us,
             NEW.received_at, NEW.token_id)
            IS NOT DISTINCT FROM
            (OLD.id, OLD.project_id, OLD.release_id, OLD.environment, OLD.decision_key,
             OLD.reference, OLD.batch_id, OLD.status, OLD.outcome, OLD.evaluated_at, OLD.duration_us,
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
