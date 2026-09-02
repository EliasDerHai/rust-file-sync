CREATE TABLE IF NOT EXISTS location_point (
	id                      INTEGER PRIMARY KEY AUTOINCREMENT,
	created_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	timestamp_epoch_ms      INTEGER  NOT NULL,
	latitude                REAL     NOT NULL,
	longitude               REAL     NOT NULL,
	altitude_meters         REAL,
	accuracy_meters         REAL,
	speed_meters_per_second REAL
);

CREATE INDEX idx_location_point_timestamp ON location_point(timestamp_epoch_ms);
