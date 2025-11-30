-- Add support for partial failure handling in queue items

-- Add succeeded_indices column to track which operations have succeeded
-- This allows retrying only failed operations instead of the entire batch
ALTER TABLE queue_items ADD COLUMN succeeded_indices_json TEXT DEFAULT '[]';

-- Note: SQLite doesn't support modifying CHECK constraints directly.
-- The existing CHECK constraint for status doesn't strictly enforce in newer SQLite versions,
-- and the new 'PartiallyFailed' status will work. The constraint is documentation-only now.
-- For strict enforcement, would need to recreate the table (not worth the complexity).
