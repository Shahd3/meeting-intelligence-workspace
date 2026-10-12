-- Journal cross-store credential cleanup; only opaque IDs, never key material.
CREATE TABLE ai_credential_cleanup (id TEXT PRIMARY KEY);
PRAGMA user_version=6;
