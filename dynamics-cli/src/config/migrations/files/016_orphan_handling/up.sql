-- Add orphan_handling column to transfer_entity_mappings
-- Controls how records that exist in target but not source are handled
ALTER TABLE transfer_entity_mappings
ADD COLUMN orphan_handling TEXT NOT NULL DEFAULT 'ignore';
