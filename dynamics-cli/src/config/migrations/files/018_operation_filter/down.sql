-- Revert operation filter columns
-- Note: SQLite doesn't support DROP COLUMN directly, but since we kept orphan_handling,
-- we can just update it from the new columns before they become inaccessible

-- Migrate new column data back to orphan_handling
UPDATE transfer_entity_mappings SET orphan_handling = 'delete' WHERE allow_deletes = 1;
UPDATE transfer_entity_mappings SET orphan_handling = 'deactivate' WHERE allow_deactivates = 1 AND allow_deletes = 0;
UPDATE transfer_entity_mappings SET orphan_handling = 'ignore' WHERE allow_deletes = 0 AND allow_deactivates = 0;

-- SQLite workaround: recreate table without new columns
CREATE TABLE transfer_entity_mappings_backup AS
SELECT id, config_id, source_entity, target_entity, priority, orphan_handling, created_at
FROM transfer_entity_mappings;

DROP TABLE transfer_entity_mappings;

CREATE TABLE transfer_entity_mappings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_id INTEGER NOT NULL,
    source_entity TEXT NOT NULL,
    target_entity TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 1,
    orphan_handling TEXT NOT NULL DEFAULT 'ignore',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (config_id) REFERENCES transfer_configs(id) ON DELETE CASCADE,
    UNIQUE(config_id, source_entity)
);

INSERT INTO transfer_entity_mappings (id, config_id, source_entity, target_entity, priority, orphan_handling, created_at)
SELECT id, config_id, source_entity, target_entity, priority, orphan_handling, created_at
FROM transfer_entity_mappings_backup;

DROP TABLE transfer_entity_mappings_backup;

CREATE INDEX idx_transfer_entity_mappings_config ON transfer_entity_mappings(config_id);
