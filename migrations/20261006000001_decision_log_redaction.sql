-- Explanations (DNK-19): per project, the fields removed from a decision record
-- before it is sent to the customer's LLM endpoint to be explained, as dotted
-- paths (`applicant.nationalId`). Checked by Studio; at most 50.
ALTER TABLE decision_log_settings
    ADD COLUMN redacted_fields text[] NOT NULL DEFAULT '{}'
        CHECK (cardinality(redacted_fields) <= 50);

