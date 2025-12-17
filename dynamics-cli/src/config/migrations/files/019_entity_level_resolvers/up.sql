-- Move resolvers from config-level to entity-level
-- Drop old table (no production data to migrate)
DROP TABLE IF EXISTS transfer_resolvers;

-- Create new table with entity_mapping_id instead of config_id
CREATE TABLE transfer_resolvers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    source_entity TEXT NOT NULL,
    match_fields_json TEXT NOT NULL,
    fallback TEXT NOT NULL DEFAULT 'error',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (entity_mapping_id) REFERENCES transfer_entity_mappings(id) ON DELETE CASCADE,
    UNIQUE(entity_mapping_id, name)
);

CREATE INDEX idx_transfer_resolvers_entity_mapping ON transfer_resolvers(entity_mapping_id);
