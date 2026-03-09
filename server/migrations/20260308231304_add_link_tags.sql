-- Recreate link with url as primary key (drop integer id), preserving existing rows
CREATE TABLE link_new (
    url        TEXT     NOT NULL PRIMARY KEY,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    name       TEXT
);

INSERT INTO link_new (url, created_at, name)
SELECT url, created_at, title FROM link;

DROP TRIGGER IF EXISTS link_updated_at;
DROP TABLE link;
ALTER TABLE link_new RENAME TO link;

CREATE TABLE link_tag (
    name       TEXT     NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    link_url   TEXT     NOT NULL,
    PRIMARY KEY (name, link_url),
    FOREIGN KEY (link_url) REFERENCES link(url) ON DELETE CASCADE
);
