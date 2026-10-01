-- Releases, environments, Runtime tokens and deployments (DNK-14).
--
-- A release freezes the latest version of every decision of a project, with a
-- semantic version and notes; its decisions are copied here so a release is
-- complete on its own and never changes. Each project has two environments,
-- staging and production (a fixed set, not a table). A deployment asks for a
-- release to be published to an environment: the row is the outbox the
-- publisher works through after commit, retrying a failed write.

CREATE TABLE releases (
    id           uuid PRIMARY KEY,
    project_id   uuid NOT NULL REFERENCES projects (id),
    major        integer NOT NULL CHECK (major >= 0),
    minor        integer NOT NULL CHECK (minor >= 0),
    patch        integer NOT NULL CHECK (patch >= 0),
    notes        text NOT NULL CHECK (char_length(btrim(notes)) BETWEEN 1 AND 2000),
    created_by   uuid NOT NULL REFERENCES users (id),
    created_at   timestamptz NOT NULL,
    UNIQUE (project_id, major, minor, patch)
);

CREATE TABLE release_decisions (
    release_id     uuid NOT NULL REFERENCES releases (id),
    decision_id    uuid NOT NULL,
    key            text NOT NULL,
    version_number integer NOT NULL,
    content        jsonb NOT NULL,
    -- How the project's scenarios went on that version, when the release was made.
    tests_passed   bigint NOT NULL,
    tests_failed   bigint NOT NULL,
    tests_errors   bigint NOT NULL,
    PRIMARY KEY (release_id, key),
    FOREIGN KEY (decision_id, version_number) REFERENCES decision_versions (decision_id, number)
);

CREATE TRIGGER releases_append_only
    BEFORE UPDATE OR DELETE ON releases
    FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation();
CREATE TRIGGER releases_no_truncate
    BEFORE TRUNCATE ON releases
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
CREATE TRIGGER release_decisions_append_only
    BEFORE UPDATE OR DELETE ON release_decisions
    FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation();
CREATE TRIGGER release_decisions_no_truncate
    BEFORE TRUNCATE ON release_decisions
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
REVOKE UPDATE, DELETE, TRUNCATE ON releases, release_decisions FROM CURRENT_USER;

-- A token lets a customer system call the Runtime for one environment of one
-- project. Only its SHA-256 is kept (docs/artifact-format.md); it is shown once.
CREATE TABLE runtime_tokens (
    id          uuid PRIMARY KEY,
    project_id  uuid NOT NULL REFERENCES projects (id),
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
CREATE INDEX runtime_tokens_environment_idx ON runtime_tokens (project_id, environment);

CREATE TABLE deployments (
    id              uuid PRIMARY KEY,
    -- The order deployments were asked for, even within the same instant.
    seq             bigint GENERATED ALWAYS AS IDENTITY,
    project_id      uuid NOT NULL REFERENCES projects (id),
    environment     text NOT NULL CHECK (environment IN ('staging', 'production')),
    release_id      uuid NOT NULL REFERENCES releases (id),
    -- The project as the artifact names it, read when the deployment is asked for.
    project_key     text NOT NULL,
    project_name    text NOT NULL,
    -- 'deploy': someone deployed the release; 'tokens': the environment's
    -- tokens changed, so its release is published again with the new hashes.
    reason          text NOT NULL CHECK (reason IN ('deploy', 'tokens')),
    requested_by    uuid NOT NULL REFERENCES users (id),
    requested_at    timestamptz NOT NULL,
    attempts        integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL,
    last_error      text,
    published_at    timestamptz,
    -- Given up after too many failed writes (it can be retried by hand).
    abandoned_at    timestamptz,
    -- A newer deployment of the same environment replaced it before it was published.
    superseded_at   timestamptz
);
CREATE INDEX deployments_due_idx ON deployments (next_attempt_at)
    WHERE published_at IS NULL AND abandoned_at IS NULL AND superseded_at IS NULL;
CREATE INDEX deployments_environment_idx ON deployments (project_id, environment, seq DESC);
