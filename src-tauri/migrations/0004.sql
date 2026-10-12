CREATE TABLE transcription_settings (
  id INTEGER PRIMARY KEY CHECK(id=1), config TEXT
);
INSERT INTO transcription_settings(id) VALUES (1);
CREATE TABLE processing_jobs (
  id TEXT PRIMARY KEY,
  recording_id TEXT NOT NULL UNIQUE REFERENCES recordings(id),
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK(state IN ('queued','processing','completed','failed','retryable')),
  stage TEXT NOT NULL DEFAULT 'waiting',
  attempts INTEGER NOT NULL DEFAULT 0,
  error TEXT, error_category TEXT,
  created_at INTEGER NOT NULL DEFAULT (unixepoch()), updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
  started_at INTEGER, completed_at INTEGER, next_attempt_at INTEGER,
  owner TEXT, config TEXT
);
CREATE INDEX processing_due ON processing_jobs(state,next_attempt_at);
CREATE TABLE transcription_checkpoints (
  job_id TEXT NOT NULL REFERENCES processing_jobs(id) ON DELETE CASCADE,
  source TEXT NOT NULL CHECK(source IN ('microphone','system')),
  chunk INTEGER NOT NULL,
  fingerprint TEXT NOT NULL,
  segments TEXT NOT NULL,
  PRIMARY KEY(job_id,source,chunk)
);
CREATE TABLE transcripts (
  job_id TEXT PRIMARY KEY REFERENCES processing_jobs(id) ON DELETE CASCADE,
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  recording_id TEXT NOT NULL REFERENCES recordings(id),
  provider TEXT NOT NULL,
  segments TEXT NOT NULL,
  warning TEXT,
  created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
PRAGMA user_version=4;
