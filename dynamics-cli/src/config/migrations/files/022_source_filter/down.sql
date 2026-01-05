-- SQLite doesn't support DROP COLUMN directly, need to recreate table
-- This is a destructive migration - source_filter data will be lost

CREATE TABLE transfer_entity_mappings_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_id INTEGER NOT NULL REFERENCES transfer_configs(id) ON DELETE CASCADE,
    source_entity TEXT NOT NULL,
    target_entity TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    allow_creates INTEGER NOT NULL DEFAULT 1,
    allow_updates INTEGER NOT NULL DEFAULT 1,
    allow_deletes INTEGER NOT NULL DEFAULT 0,
    allow_deactivates INTEGER NOT NULL DEFAULT 0
);

INSERT INTO transfer_entity_mappings_new (id, config_id, source_entity, target_entity, priority, allow_creates, allow_updates, allow_deletes, allow_deactivates)
SELECT id, config_id, source_entity, target_entity, priority, allow_creates, allow_updates, allow_deletes, allow_deactivates
FROM transfer_entity_mappings;

DROP TABLE transfer_entity_mappings;
ALTER TABLE transfer_entity_mappings_new RENAME TO transfer_entity_mappings;
