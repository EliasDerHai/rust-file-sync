CREATE TABLE link_tag (
    name        TEXT     NOT NULL,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    link_id     INTEGER  NOT NULL,
    PRIMARY KEY (name, link_id),
    FOREIGN KEY (link_id) REFERENCES link(id) ON DELETE CASCADE
);
