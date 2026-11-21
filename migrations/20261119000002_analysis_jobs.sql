-- Decision impact analysis retains aggregate results and changed record identifiers only.
CREATE TABLE analysis_jobs (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    approval_id uuid NOT NULL REFERENCES approvals (id) ON DELETE CASCADE,
    release_id uuid NOT NULL REFERENCES releases (id) ON DELETE CASCADE,
    decision_key text NOT NULL CHECK (char_length(decision_key) BETWEEN 1 AND 200),
    started_by uuid NOT NULL REFERENCES users (id),
    from_at timestamptz NOT NULL,
    until_at timestamptz NOT NULL,
    status text NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'cancelled', 'failed')),
    records_read integer NOT NULL DEFAULT 0 CHECK (records_read >= 0),
    result jsonb,
    cancel_requested boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL,
    finished_at timestamptz,
    CHECK (from_at < until_at)
);
CREATE INDEX analysis_jobs_project_recent_idx ON analysis_jobs (project_id, created_at DESC);
CREATE INDEX analysis_jobs_queued_idx ON analysis_jobs (created_at) WHERE status = 'queued';
CREATE UNIQUE INDEX analysis_jobs_one_running_per_project_idx
    ON analysis_jobs (project_id) WHERE status = 'running';

-- Removing/anonymising a record must remove any stored reference to it.
CREATE TABLE analysis_job_records (
    job_id uuid NOT NULL REFERENCES analysis_jobs (id) ON DELETE CASCADE,
    record_id uuid NOT NULL,
    evaluated_at timestamptz NOT NULL,
    PRIMARY KEY (job_id, record_id, evaluated_at)
);
CREATE INDEX analysis_job_records_record_idx ON analysis_job_records (record_id, evaluated_at);
