-- Projects and who can work on them (DNK-7).

CREATE TABLE projects (
    id          uuid PRIMARY KEY,
    -- Immutable, URL-safe identifier; release artifacts are stored under it (DNK-14).
    key         text NOT NULL,
    name        text NOT NULL,
    description text NOT NULL DEFAULT '',
    created_by  uuid NOT NULL REFERENCES users (id),
    created_at  timestamptz NOT NULL,
    -- Archived projects are read-only and hidden from the default list.
    archived_at timestamptz,
    CONSTRAINT projects_key_format
        CHECK (char_length(key) BETWEEN 2 AND 40 AND key ~ '^[a-z][a-z0-9]*(-[a-z0-9]+)*$'),
    CONSTRAINT projects_name_length CHECK (char_length(btrim(name)) BETWEEN 1 AND 100),
    CONSTRAINT projects_description_length CHECK (char_length(description) <= 1000)
);
CREATE UNIQUE INDEX projects_key_key ON projects (key);

CREATE TABLE project_members (
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role       text NOT NULL CHECK (role IN ('owner', 'editor', 'viewer')),
    added_at   timestamptz NOT NULL,
    PRIMARY KEY (project_id, user_id)
);
CREATE INDEX project_members_user_id_idx ON project_members (user_id);
