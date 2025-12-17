-- Revert to config-level resolvers
DROP TABLE IF EXISTS transfer_resolvers;

-- Recreate old structure with config_id
CREATE TABLE transfer_resolvers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    source_entity TEXT NOT NULL,
    match_field TEXT NOT NULL,
    fallback TEXT NOT NULL DEFAULT 'error',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (config_id) REFERENCES transfer_configs(id) ON DELETE CASCADE,
    UNIQUE(config_id, name)
);

CREATE INDEX idx_transfer_resolvers_config ON transfer_resolvers(config_id);
