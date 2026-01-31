DROP TABLE shared_link;

CREATE TABLE link (
	id		INTEGER PRIMARY KEY,
	created_at	DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at	DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	url 		TEXT NOT NULL,
	title 		TEXT
);

-- Auto-update updated_at on row modification
CREATE TRIGGER link_updated_at
AFTER UPDATE ON link 
FOR EACH ROW
BEGIN
	UPDATE link SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;
