-- World metadata storage for persistent identity and world-level configuration.

CREATE TABLE world_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT (datetime('now'))
);
