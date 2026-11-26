-- Audit log (DNK-11): one row per state change, written in the transaction of
-- the change, never updated or deleted.

CREATE TABLE audit_events (
    id             bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    occurred_at    timestamptz NOT NULL,
    -- Who did it; NULL for changes Studio makes on its own.
    actor_id       uuid REFERENCES users (id),
    -- e.g. 'project.created', 'member.role_changed' (crates/audit `Action`).
    action         text NOT NULL,
    -- The project it happened in; NULL for account events (sign-in, invitations).
    project_id     uuid REFERENCES projects (id),
    -- The person the change is about, when it is not the actor (member added…).
    target_user_id uuid REFERENCES users (id),
    -- What changed, e.g. {"from": "viewer", "to": "editor"}. Never secrets.
    details        jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX audit_events_project_idx ON audit_events (project_id, occurred_at DESC, id DESC);
CREATE INDEX audit_events_actor_idx ON audit_events (actor_id, occurred_at DESC);

-- Two independent guards: the trigger refuses changes from anyone, and the
-- application role gives up the privileges (it could only get them back by an
-- explicit, visible GRANT).
CREATE TRIGGER audit_events_append_only
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION donka_reject_mutation();
CREATE TRIGGER audit_events_no_truncate
    BEFORE TRUNCATE ON audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION donka_reject_mutation();
REVOKE UPDATE, DELETE, TRUNCATE ON audit_events FROM CURRENT_USER;
