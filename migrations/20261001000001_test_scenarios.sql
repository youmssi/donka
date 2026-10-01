-- Test scenarios and their results on each version (DNK-10).
--
-- A scenario is an input to one decision and the output it should give, exactly
-- or in part. Every version saved runs every scenario of the project; the
-- results are kept with that version and never change, so an approver sees
-- what the version did when it was saved.

CREATE TABLE test_scenarios (
    id          uuid PRIMARY KEY,
    project_id  uuid NOT NULL REFERENCES projects (id),
    decision_id uuid NOT NULL REFERENCES decisions (id),
    name        text NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 200),
    input       jsonb NOT NULL CHECK (jsonb_typeof(input) = 'object'),
    expected    jsonb NOT NULL CHECK (jsonb_typeof(expected) = 'object'),
    match       text NOT NULL CHECK (match IN ('exact', 'partial')),
    created_by  uuid NOT NULL REFERENCES users (id),
    created_at  timestamptz NOT NULL,
    updated_by  uuid NOT NULL REFERENCES users (id),
    updated_at  timestamptz NOT NULL
);

-- Names tell the scenarios of a decision apart.
CREATE UNIQUE INDEX test_scenarios_decision_name_key ON test_scenarios (decision_id, lower(name));
CREATE INDEX test_scenarios_project_idx ON test_scenarios (project_id);

-- One row per scenario run on a version. The scenario is copied as it was when
-- it ran: editing or deleting it later does not rewrite what was tested.
CREATE TABLE test_results (
    decision_id    uuid NOT NULL,
    version_number integer NOT NULL,
    scenario_id    uuid NOT NULL,
    name           text NOT NULL,
    decision_key   text NOT NULL,
    input          jsonb NOT NULL,
    expected       jsonb NOT NULL,
    match          text NOT NULL,
    status         text NOT NULL CHECK (status IN ('passed', 'failed', 'error')),
    -- What the decision returned (absent when it could not run).
    actual         jsonb,
    -- Per field: {path, expected, actual}; empty when it passed.
    mismatches     jsonb NOT NULL,
    -- Why it could not run (status 'error'): a decision without a saved
    -- version (its key), or the engine's message.
    missing_decision text,
    error          text,
    PRIMARY KEY (decision_id, version_number, scenario_id),
    FOREIGN KEY (decision_id, version_number) REFERENCES decision_versions (decision_id, number)
);

CREATE TRIGGER test_results_append_only
    BEFORE UPDATE OR DELETE ON test_results
    FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation();
CREATE TRIGGER test_results_no_truncate
    BEFORE TRUNCATE ON test_results
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
REVOKE UPDATE, DELETE, TRUNCATE ON test_results FROM CURRENT_USER;
