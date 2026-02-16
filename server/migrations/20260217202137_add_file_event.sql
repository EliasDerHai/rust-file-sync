CREATE TABLE IF NOT EXISTS file_event (
	id                      TEXT PRIMARY KEY,  -- UUID
	created_at		DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at		DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	utc_millis		INTEGER  NOT NULL,
	relative_path		TEXT 	 NOT NULL,
	size_in_bytes		INTEGER  NOT NULL,
	event_type 		TEXT 	 NOT NULL CHECK (event_type IN ('change', 'delete')),
	client_id 		TEXT 	 NOT NULL REFERENCES client(id),
	watch_group_id		INTEGER  NOT NULL REFERENCES server_watch_group(id)
);

-- Auto-update updated_at on row modification
CREATE TRIGGER file_event_updated_at
AFTER UPDATE ON file_event 
FOR EACH ROW
BEGIN
	UPDATE file_event SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;
