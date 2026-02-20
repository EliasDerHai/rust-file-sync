-- SQLite does not report notnull=1 for PRIMARY KEY columns in pragma_table_info unless NOT NULL is stated explicitly.

PRAGMA foreign_keys = OFF;

-- client ------------------------------------------------------------------
CREATE TABLE client_new (
    id                      TEXT     PRIMARY KEY NOT NULL,
    created_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    host_name               TEXT     NOT NULL,
    min_poll_interval_in_ms INTEGER  NOT NULL DEFAULT 3000
);
INSERT INTO client_new SELECT * FROM client;
DROP TABLE client;
ALTER TABLE client_new RENAME TO client;

-- server_watch_group ------------------------------------------------------
CREATE TABLE server_watch_group_new (
    id         INTEGER  PRIMARY KEY NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    name       TEXT     NOT NULL UNIQUE
);
INSERT INTO server_watch_group_new SELECT * FROM server_watch_group;
DROP TABLE server_watch_group;
ALTER TABLE server_watch_group_new RENAME TO server_watch_group;

-- client_watch_group ------------------------------------------------------
CREATE TABLE client_watch_group_new (
    id                    INTEGER  PRIMARY KEY NOT NULL,
    created_at            DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at            DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    client_id             TEXT     NOT NULL REFERENCES client(id) ON DELETE CASCADE,
    path_to_monitor       TEXT     NOT NULL,
    exclude_dot_dirs      BOOLEAN  NOT NULL DEFAULT TRUE,
    server_watch_group_id INTEGER  NOT NULL DEFAULT 1
);
INSERT INTO client_watch_group_new SELECT * FROM client_watch_group;
DROP TABLE client_watch_group;
ALTER TABLE client_watch_group_new RENAME TO client_watch_group;

-- client_watch_group_excluded_dir -----------------------------------------
CREATE TABLE client_watch_group_excluded_dir_new (
    id                 INTEGER PRIMARY KEY NOT NULL,
    client_watch_group INTEGER NOT NULL REFERENCES client_watch_group(id) ON DELETE CASCADE,
    exclude_dir        TEXT    NOT NULL
);
INSERT INTO client_watch_group_excluded_dir_new SELECT * FROM client_watch_group_excluded_dir;
DROP TABLE client_watch_group_excluded_dir;
ALTER TABLE client_watch_group_excluded_dir_new RENAME TO client_watch_group_excluded_dir;

-- link --------------------------------------------------------------------
CREATE TABLE link_new (
    id         INTEGER  PRIMARY KEY NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    url        TEXT     NOT NULL,
    title      TEXT
);
INSERT INTO link_new SELECT * FROM link;
DROP TABLE link;
ALTER TABLE link_new RENAME TO link;

-- file_event --------------------------------------------------------------
CREATE TABLE file_event_new (
    id             TEXT     PRIMARY KEY NOT NULL,
    created_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    utc_millis     INTEGER  NOT NULL,
    relative_path  TEXT     NOT NULL,
    size_in_bytes  INTEGER  NOT NULL,
    event_type     TEXT     NOT NULL CHECK (event_type IN ('change', 'delete')),
    client_id      TEXT     NOT NULL REFERENCES client(id),
    watch_group_id INTEGER  NOT NULL REFERENCES server_watch_group(id)
);
INSERT INTO file_event_new SELECT * FROM file_event;
DROP TABLE file_event;
ALTER TABLE file_event_new RENAME TO file_event;

PRAGMA foreign_keys = ON;
PRAGMA foreign_key_check;

-- Recreate triggers -------------------------------------------------------
CREATE TRIGGER client_updated_at
AFTER UPDATE ON client
FOR EACH ROW
BEGIN
    UPDATE client SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

CREATE TRIGGER client_watch_group_updated_at
AFTER UPDATE ON client_watch_group
FOR EACH ROW
BEGIN
    UPDATE client_watch_group SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

CREATE TRIGGER link_updated_at
AFTER UPDATE ON link
FOR EACH ROW
BEGIN
    UPDATE link SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

CREATE TRIGGER server_watch_group_updated_at
AFTER UPDATE ON server_watch_group
FOR EACH ROW
BEGIN
    UPDATE server_watch_group SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

CREATE TRIGGER file_event_updated_at
AFTER UPDATE ON file_event
FOR EACH ROW
BEGIN
    UPDATE file_event SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

-- Recreate indexes ---------------------------------------------------------
CREATE UNIQUE INDEX uq_client_watch_group_client_server
    ON client_watch_group(client_id, server_watch_group_id);

CREATE UNIQUE INDEX uq_client_watch_group_client_path
    ON client_watch_group(client_id, path_to_monitor);
