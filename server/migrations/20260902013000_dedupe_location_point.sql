-- Keep the earliest-inserted row per timestamp, drop the rest.
DELETE FROM location_point
WHERE id NOT IN (
    SELECT MIN(id) FROM location_point GROUP BY timestamp_epoch_ms
);

-- Replace the existing non-unique index with a UNIQUE one on the same column -
-- enforces "one point per millisecond" at the DB level going forward.
DROP INDEX IF EXISTS idx_location_point_timestamp;
CREATE UNIQUE INDEX idx_location_point_timestamp ON location_point(timestamp_epoch_ms);
