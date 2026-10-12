CREATE TABLE recordings (
  id TEXT PRIMARY KEY,
  meeting_id INTEGER REFERENCES meetings(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('starting','recording','paused','stopping','saved','failed')),
  elapsed_ms INTEGER NOT NULL DEFAULT 0 CHECK(elapsed_ms BETWEEN 0 AND 14400000),
  recovered INTEGER NOT NULL DEFAULT 0 CHECK(recovered IN (0,1)),
  complete INTEGER NOT NULL DEFAULT 0 CHECK(complete IN (0,1)),
  finalized INTEGER NOT NULL DEFAULT 0 CHECK(finalized IN (0,1)),
  warning TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX recording_meeting ON recordings(meeting_id);
CREATE TABLE recording_tracks (
  recording_id TEXT NOT NULL REFERENCES recordings(id),
  role TEXT NOT NULL CHECK(role IN ('microphone','system')),
  device_id TEXT NOT NULL,
  device_name TEXT NOT NULL,
  filename TEXT NOT NULL,
  frames INTEGER NOT NULL DEFAULT 0 CHECK(frames BETWEEN 0 AND 691200000),
  sample_rate INTEGER NOT NULL DEFAULT 48000 CHECK(sample_rate=48000),
  channels INTEGER NOT NULL DEFAULT 2 CHECK(channels=2),
  PRIMARY KEY(recording_id,role)
);
PRAGMA user_version=2;
