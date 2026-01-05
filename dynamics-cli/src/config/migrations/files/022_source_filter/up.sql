-- Add source_filter_json column to transfer_entity_mappings
ALTER TABLE transfer_entity_mappings ADD COLUMN source_filter_json TEXT;
