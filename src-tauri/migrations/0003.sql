CREATE TABLE desktop_preferences (
  id INTEGER PRIMARY KEY CHECK(id=1),
  meeting_detection INTEGER NOT NULL DEFAULT 0 CHECK(meeting_detection IN (0,1))
);
INSERT INTO desktop_preferences(id,meeting_detection) VALUES(1,0);
PRAGMA user_version=3;
