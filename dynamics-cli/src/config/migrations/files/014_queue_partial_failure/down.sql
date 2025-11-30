-- Revert partial failure handling

-- SQLite doesn't support DROP COLUMN in older versions, but newer versions do.
-- For compatibility, we'll just leave the column (it won't break anything).
-- If using SQLite 3.35.0+, this will work:
-- ALTER TABLE queue_items DROP COLUMN succeeded_indices_json;

-- Update any PartiallyFailed items back to Failed
UPDATE queue_items SET status = 'Failed' WHERE status = 'PartiallyFailed';
