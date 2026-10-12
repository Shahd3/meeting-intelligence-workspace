CREATE TABLE transcription_preferences (
  id INTEGER PRIMARY KEY CHECK(id=1),
  automatic_revision TEXT,
  after_recording_rowid INTEGER NOT NULL DEFAULT 0
);
INSERT INTO transcription_preferences(id) VALUES (1);
ALTER TABLE processing_jobs ADD COLUMN ready INTEGER NOT NULL DEFAULT 0 CHECK(ready IN (0,1));
-- Preserve work already started/authorized; unstarted backlog stays manual.
UPDATE processing_jobs SET ready=1 WHERE state IN ('processing','completed','retryable')
  OR (config IS NOT NULL AND COALESCE(error_category,'')!='consent');
UPDATE processing_jobs SET stage='manual' WHERE state='queued' AND ready=0;
PRAGMA user_version=7;
