-- The optional modules the installation turns on (DNK-74, `DONKA_MODULES`), as Studio last
-- started with them. Studio compares at each start and audits a change; a module turned off
-- keeps its data. No row, or a NULL list: every module the licence includes.
CREATE TABLE installation_modules (
    only_row   boolean PRIMARY KEY DEFAULT true CHECK (only_row),
    modules    text[],
    applied_at timestamptz NOT NULL
);
