-- The installation itself (DNK-74): the organisation it serves, and whether its first-run
-- assistant is done. One row at most; none until the first administrator finishes the
-- assistant in the browser.
CREATE TABLE installation (
    only_row     boolean PRIMARY KEY DEFAULT true CHECK (only_row),
    -- Shown in Studio's header, its emails, the person access report and audit exports.
    organisation text CHECK (char_length(btrim(organisation)) BETWEEN 1 AND 100),
    set_up_by    uuid REFERENCES users (id),
    set_up_at    timestamptz NOT NULL
);

-- An installation someone already signs in to was set up before the assistant existed: it is
-- not offered again. Its administrators name the organisation in Studio's settings.
INSERT INTO installation (set_up_at)
SELECT now() WHERE EXISTS (SELECT 1 FROM users WHERE password_hash IS NOT NULL);
