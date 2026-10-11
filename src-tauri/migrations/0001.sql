CREATE TABLE workspaces (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 50),
  description TEXT NOT NULL,
  color TEXT NOT NULL CHECK(color IN ('pink','sage','lav','yellow','peach')),
  symbol TEXT NOT NULL, overview TEXT NOT NULL, decision TEXT,
  task TEXT NOT NULL, owner TEXT NOT NULL, due TEXT NOT NULL
);
CREATE TABLE meetings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  workspace_id INTEGER REFERENCES workspaces(id) ON DELETE SET NULL,
  title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 90),
  date TEXT NOT NULL, duration TEXT NOT NULL, summary TEXT NOT NULL,
  highlights TEXT NOT NULL CHECK(json_valid(highlights)),
  decision TEXT NOT NULL,
  actions TEXT NOT NULL CHECK(json_valid(actions)),
  speakers TEXT NOT NULL CHECK(json_valid(speakers))
);
CREATE INDEX meetings_by_workspace ON meetings(workspace_id);
CREATE TABLE workspace_completion (
  workspace_id INTEGER PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  done INTEGER NOT NULL CHECK(done IN (0,1))
);
CREATE TABLE meeting_completion (
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  action_index INTEGER NOT NULL CHECK(action_index >= 0),
  done INTEGER NOT NULL CHECK(done IN (0,1)),
  PRIMARY KEY(meeting_id,action_index)
);
PRAGMA user_version=1;
