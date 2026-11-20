-- DNK-71: record why a production release reads fields the project marked protected.
ALTER TABLE approvals
    ADD COLUMN justification text
    CHECK (justification IS NULL OR char_length(btrim(justification)) BETWEEN 1 AND 1000);
