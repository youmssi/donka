-- First-run onboarding (DNK-41). The Get started checklist reads what already happened in a
-- project; the one fact Studio did not keep is whether a simulation ever ran, recorded here
-- the first time one succeeds. Tours seen are kept per user so they do not replay on another
-- device.
CREATE TABLE decision_simulations (
    project_id uuid PRIMARY KEY REFERENCES projects (id),
    first_at   timestamptz NOT NULL
);

CREATE TABLE user_tours (
    user_id uuid NOT NULL REFERENCES users (id),
    tour    text NOT NULL CHECK (tour IN ('editor', 'releases', 'environments')),
    seen_at timestamptz NOT NULL,
    PRIMARY KEY (user_id, tour)
);
