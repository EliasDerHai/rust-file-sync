CREATE TABLE IF NOT EXISTS server_watch_group (
	id			INTEGER PRIMARY KEY,
	created_at		DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at		DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	name			TEXT NOT NULL UNIQUE
);

CREATE TRIGGER server_watch_group_updated_at
AFTER UPDATE ON server_watch_group
FOR EACH ROW
BEGIN
	UPDATE server_watch_group SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

INSERT INTO server_watch_group (name) VALUES ('default');

ALTER TABLE client_watch_group
	ADD COLUMN server_watch_group_id INTEGER NOT NULL DEFAULT 1;

CREATE UNIQUE INDEX uq_client_watch_group_client_server
	ON client_watch_group(client_id, server_watch_group_id);

CREATE UNIQUE INDEX uq_client_watch_group_client_path
	ON client_watch_group(client_id, path_to_monitor);
