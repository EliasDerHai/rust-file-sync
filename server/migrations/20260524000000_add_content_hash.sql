-- Add with DEFAULT 0 so existing rows are backfilled
ALTER TABLE file_event ADD COLUMN content_hash INTEGER NOT NULL DEFAULT 0;

-- Rebuild to drop the default, keeping NOT NULL
PRAGMA foreign_keys = OFF;

CREATE TABLE file_event_new (
    id             TEXT     PRIMARY KEY NOT NULL,
    created_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    utc_millis     INTEGER  NOT NULL,
    relative_path  TEXT     NOT NULL,
    size_in_bytes  INTEGER  NOT NULL,
    content_hash   INTEGER  NOT NULL,
    event_type     TEXT     NOT NULL CHECK (event_type IN ('change', 'delete')),
    client_id      TEXT     NOT NULL REFERENCES client(id),
    watch_group_id INTEGER  NOT NULL REFERENCES server_watch_group(id)
);

INSERT INTO file_event_new
    SELECT id, created_at, updated_at, utc_millis, relative_path, size_in_bytes, content_hash, event_type, client_id, watch_group_id
    FROM file_event;

DROP TABLE file_event;
ALTER TABLE file_event_new RENAME TO file_event;

PRAGMA foreign_keys = ON;
PRAGMA foreign_key_check;

CREATE TRIGGER file_event_updated_at
AFTER UPDATE ON file_event
FOR EACH ROW
BEGIN
    UPDATE file_event SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;
