-- Soft-delete marker for outlier GPS points reviewed and confirmed for removal in
-- the web admin's Locations tab. NULL = active; set = excluded from GET responses
-- but kept in the DB (this is irreplaceable travel history, never hard-deleted).
ALTER TABLE location_point ADD COLUMN excluded_at DATETIME NULL;
