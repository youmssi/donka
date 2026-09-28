-- Reusable guard for tables whose rows must never change once written
-- (audit events, decision log). Attach it with:
--
--   CREATE TRIGGER <table>_append_only
--       BEFORE UPDATE OR DELETE ON <table>
--       FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation();
--   CREATE TRIGGER <table>_no_truncate
--       BEFORE TRUNCATE ON <table>
--       FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
--
-- The application role is also denied UPDATE, DELETE and TRUNCATE on those
-- tables; the trigger protects against anything that still has the privilege.

CREATE FUNCTION donka_reject_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'table % is append-only: % is not allowed', TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;
