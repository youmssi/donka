-- Decisions of a project and their working drafts (DNK-8).
--
-- A decision is one JDM graph, addressed by a key that graphs use to call each
-- other (`person-score`, `bureau/normalize`). `content` is the draft the editor
-- autosaves; immutable versions come with DNK-9. `revision` grows by one on every
-- save, so two people saving the same draft get a conflict instead of one
-- silently overwriting the other.

CREATE TABLE decisions (
    id         uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key        text NOT NULL,
    content    jsonb NOT NULL,
    revision   integer NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_by uuid NOT NULL REFERENCES users (id),
    created_at timestamptz NOT NULL,
    updated_by uuid NOT NULL REFERENCES users (id),
    updated_at timestamptz NOT NULL,
    CONSTRAINT decisions_key_format CHECK (
        char_length(key) BETWEEN 1 AND 120
        AND key ~ '^[a-z][a-z0-9]*(-[a-z0-9]+)*(/[a-z][a-z0-9]*(-[a-z0-9]+)*)*$'
    )
);
CREATE UNIQUE INDEX decisions_project_key_key ON decisions (project_id, key);
