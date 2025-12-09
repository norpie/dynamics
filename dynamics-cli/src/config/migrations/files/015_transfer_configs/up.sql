-- Transfer configuration tables for data migration between environments

-- Top-level transfer configuration
CREATE TABLE transfer_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    source_env TEXT NOT NULL,
    target_env TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_env) REFERENCES environments(name) ON DELETE CASCADE,
    FOREIGN KEY (target_env) REFERENCES environments(name) ON DELETE CASCADE
);

CREATE INDEX idx_transfer_configs_source_env ON transfer_configs(source_env);
CREATE INDEX idx_transfer_configs_target_env ON transfer_configs(target_env);

-- Entity mappings (source entity -> target entity)
CREATE TABLE transfer_entity_mappings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_id INTEGER NOT NULL,
    source_entity TEXT NOT NULL,
    target_entity TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (config_id) REFERENCES transfer_configs(id) ON DELETE CASCADE,
    UNIQUE(config_id, source_entity)
);

CREATE INDEX idx_transfer_entity_mappings_config ON transfer_entity_mappings(config_id);

-- Field mappings with transform stored as JSON
CREATE TABLE transfer_field_mappings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_mapping_id INTEGER NOT NULL,
    target_field TEXT NOT NULL,
    transform_json TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (entity_mapping_id) REFERENCES transfer_entity_mappings(id) ON DELETE CASCADE,
    UNIQUE(entity_mapping_id, target_field)
);

CREATE INDEX idx_transfer_field_mappings_entity ON transfer_field_mappings(entity_mapping_id);
