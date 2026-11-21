-- Table findings an owner acknowledged as intended, with why (DNK-76). A finding is identified by
-- its fingerprint, which stays the same while the finding does: the acknowledgement holds on
-- later versions until the table changes what it finds.
CREATE TABLE table_finding_acknowledgements (
    decision_id     uuid NOT NULL REFERENCES decisions (id) ON DELETE CASCADE,
    fingerprint     text NOT NULL,
    reason          text NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 1000),
    acknowledged_by uuid NOT NULL REFERENCES users (id),
    acknowledged_at timestamptz NOT NULL,
    PRIMARY KEY (decision_id, fingerprint)
);
