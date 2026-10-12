-- Generation is independent from transcription; existing artifacts are untouched.
CREATE TABLE intelligence_preferences (
 id INTEGER PRIMARY KEY CHECK(id=1), automatic_revision TEXT,
 after_recording_rowid INTEGER NOT NULL DEFAULT 0,
 language TEXT NOT NULL DEFAULT 'en' CHECK(language IN ('en','ar','auto')),
 format TEXT NOT NULL DEFAULT 'json_object' CHECK(format IN ('json_object','json_schema'))
);
INSERT INTO intelligence_preferences(id) VALUES(1);
CREATE TABLE transcript_revisions (
 id TEXT PRIMARY KEY REFERENCES transcripts(job_id) ON DELETE CASCADE,
 meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
 digest TEXT NOT NULL, segments TEXT NOT NULL, warning TEXT,
 created_at INTEGER NOT NULL DEFAULT(unixepoch())
);
CREATE TABLE intelligence_jobs (
 id TEXT PRIMARY KEY, transcript_id TEXT NOT NULL REFERENCES transcript_revisions(id) ON DELETE CASCADE,
 meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
 operation TEXT NOT NULL UNIQUE,
 state TEXT NOT NULL CHECK(state IN ('queued','processing','retryable','failed','completed')),
 stage TEXT NOT NULL DEFAULT 'waiting', attempts INTEGER NOT NULL DEFAULT 0,
 requests INTEGER NOT NULL DEFAULT 0, owner TEXT, config TEXT NOT NULL,
 error TEXT, error_category TEXT, next_attempt_at INTEGER,
 created_at INTEGER NOT NULL DEFAULT(unixepoch()), updated_at INTEGER NOT NULL DEFAULT(unixepoch()), completed_at INTEGER
);
CREATE INDEX intelligence_due ON intelligence_jobs(state,next_attempt_at);
CREATE UNIQUE INDEX intelligence_one_active ON intelligence_jobs(transcript_id) WHERE state IN ('queued','processing','retryable');
CREATE TABLE intelligence_checkpoints (
 job_id TEXT NOT NULL REFERENCES intelligence_jobs(id) ON DELETE CASCADE,
 stage TEXT NOT NULL, fingerprint TEXT NOT NULL, result TEXT NOT NULL,
 PRIMARY KEY(job_id,stage)
);
CREATE TABLE meeting_intelligence (
 id TEXT PRIMARY KEY REFERENCES intelligence_jobs(id) ON DELETE CASCADE,
 meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
 transcript_id TEXT NOT NULL REFERENCES transcript_revisions(id) ON DELETE CASCADE,
 result TEXT NOT NULL, coverage TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT(unixepoch())
);
CREATE TABLE intelligence_actions (
 id TEXT PRIMARY KEY, result_id TEXT NOT NULL REFERENCES meeting_intelligence(id) ON DELETE CASCADE,
 ordinal INTEGER NOT NULL, done INTEGER NOT NULL DEFAULT 0 CHECK(done IN (0,1)),
 UNIQUE(result_id,ordinal)
);
CREATE TABLE meeting_intelligence_selection (
 meeting_id INTEGER PRIMARY KEY REFERENCES meetings(id) ON DELETE CASCADE,
 result_id TEXT NOT NULL REFERENCES meeting_intelligence(id) ON DELETE CASCADE
);
CREATE TABLE intelligence_attendee_overrides (
 meeting_id INTEGER PRIMARY KEY REFERENCES meetings(id) ON DELETE CASCADE,
 names TEXT NOT NULL
);
PRAGMA user_version=8;
