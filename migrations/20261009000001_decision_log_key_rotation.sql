-- Decision-log key rotation (DNK-40). Records stay append-only, with one
-- exception: re-sealing a record with the current key. A transaction that sets
-- `donka.reseal` may change `key_id`, `nonce` and `payload` together, to
-- another key, and nothing else; the record id stays the associated data, so
-- the plaintext is the same before and after. Every other update and delete is
-- still refused.
CREATE OR REPLACE FUNCTION donka_decision_records_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE'
        AND OLD.evaluated_at < NULLIF(current_setting('donka.purge_before', true), '')::timestamptz
    THEN
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE'
        AND current_setting('donka.reseal', true) = 'on'
        AND NEW.key_id IS DISTINCT FROM OLD.key_id
        AND (NEW.id, NEW.project_id, NEW.release_id, NEW.environment, NEW.decision_key,
             NEW.reference, NEW.status, NEW.outcome, NEW.evaluated_at, NEW.duration_us,
             NEW.received_at, NEW.token_id)
            IS NOT DISTINCT FROM
            (OLD.id, OLD.project_id, OLD.release_id, OLD.environment, OLD.decision_key,
             OLD.reference, OLD.status, OLD.outcome, OLD.evaluated_at, OLD.duration_us,
             OLD.received_at, OLD.token_id)
    THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'table % is append-only: % is not allowed', TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

-- Only the sealed columns can be written, and only through the guard above.
GRANT UPDATE (key_id, nonce, payload) ON decision_records TO CURRENT_USER;

-- Re-sealing walks the records sealed with another key.
CREATE INDEX decision_records_key_idx ON decision_records (key_id, id);
