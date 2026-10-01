-- Immutable versions of a decision (DNK-9).
--
-- "Save version" snapshots the draft with its author, time and message. A
-- version never changes and is never deleted: restoring an old one adds a new
-- version with its content. Releases (DNK-14) will point at versions, so a
-- deleted decision keeps its history: decisions are now deleted softly.

ALTER TABLE decisions
    ADD COLUMN deleted_at timestamptz,
    ADD COLUMN deleted_by uuid REFERENCES users (id);

-- A key is unique among the decisions that still exist; a deleted one frees it.
DROP INDEX decisions_project_key_key;
CREATE UNIQUE INDEX decisions_project_key_key ON decisions (project_id, key) WHERE deleted_at IS NULL;

CREATE TABLE decision_versions (
    decision_id    uuid NOT NULL REFERENCES decisions (id),
    number         integer NOT NULL CHECK (number >= 1),
    content        jsonb NOT NULL,
    message        text NOT NULL CHECK (char_length(btrim(message)) BETWEEN 1 AND 500),
    -- The draft revision the version was taken from: the draft has changed
    -- since the latest version when its revision differs.
    draft_revision integer NOT NULL,
    -- Set when the version restores an older one.
    restored_from  integer,
    created_by     uuid NOT NULL REFERENCES users (id),
    created_at     timestamptz NOT NULL,
    PRIMARY KEY (decision_id, number)
);

CREATE TRIGGER decision_versions_append_only
    BEFORE UPDATE OR DELETE ON decision_versions
    FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation();
CREATE TRIGGER decision_versions_no_truncate
    BEFORE TRUNCATE ON decision_versions
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
REVOKE UPDATE, DELETE, TRUNCATE ON decision_versions FROM CURRENT_USER;
