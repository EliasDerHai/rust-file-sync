CREATE TABLE shared_link (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL,
    title TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
