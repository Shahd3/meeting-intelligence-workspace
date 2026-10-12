CREATE TABLE ai_settings (
  kind TEXT PRIMARY KEY CHECK(kind IN ('transcription','summarization')),
  config TEXT NOT NULL
);
ALTER TABLE processing_jobs ADD COLUMN upload_revision TEXT;
PRAGMA user_version=5;
