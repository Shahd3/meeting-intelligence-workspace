-- Preserve all existing titles as manual; never guess intent from their text.
ALTER TABLE meetings ADD COLUMN title_source TEXT NOT NULL DEFAULT 'manual' CHECK(title_source IN ('manual','system','ai'));
ALTER TABLE recordings ADD COLUMN title_source TEXT NOT NULL DEFAULT 'manual' CHECK(title_source IN ('manual','system','ai'));
PRAGMA user_version=9;
