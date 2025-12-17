-- Add operation filter columns to transfer_entity_mappings
-- These replace the orphan_handling column with more granular control

ALTER TABLE transfer_entity_mappings ADD COLUMN allow_creates INTEGER NOT NULL DEFAULT 1;
ALTER TABLE transfer_entity_mappings ADD COLUMN allow_updates INTEGER NOT NULL DEFAULT 1;
ALTER TABLE transfer_entity_mappings ADD COLUMN allow_deletes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transfer_entity_mappings ADD COLUMN allow_deactivates INTEGER NOT NULL DEFAULT 0;

-- Migrate existing orphan_handling data to new columns
-- 'delete' -> allow_deletes = 1
-- 'deactivate' -> allow_deactivates = 1
-- 'ignore' (default) -> both stay 0

UPDATE transfer_entity_mappings SET allow_deletes = 1 WHERE orphan_handling = 'delete';
UPDATE transfer_entity_mappings SET allow_deactivates = 1 WHERE orphan_handling = 'deactivate';

-- Note: orphan_handling column is kept for backwards compatibility
-- It will be ignored by the application going forward
