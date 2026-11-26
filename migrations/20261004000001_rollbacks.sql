-- Rolling production back (DNK-16).
--
-- An owner redeploys a release once approved for production, with a reason
-- and without a new approval. The deployment says so, and keeps the reason.

ALTER TABLE deployments DROP CONSTRAINT deployments_reason_check;
ALTER TABLE deployments
    ADD CONSTRAINT deployments_reason_check CHECK (reason IN ('deploy', 'tokens', 'rollback'));
ALTER TABLE deployments
    ADD COLUMN rollback_reason text
        CHECK (rollback_reason IS NULL OR char_length(btrim(rollback_reason)) BETWEEN 1 AND 1000),
    ADD CONSTRAINT deployments_rollback_has_reason CHECK ((reason = 'rollback') = (rollback_reason IS NOT NULL));
