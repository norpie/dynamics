-- Drop transfer configuration tables in reverse order (due to foreign keys)

DROP INDEX IF EXISTS idx_transfer_field_mappings_entity;
DROP TABLE IF EXISTS transfer_field_mappings;

DROP INDEX IF EXISTS idx_transfer_entity_mappings_config;
DROP TABLE IF EXISTS transfer_entity_mappings;

DROP INDEX IF EXISTS idx_transfer_configs_source_env;
DROP INDEX IF EXISTS idx_transfer_configs_target_env;
DROP TABLE IF EXISTS transfer_configs;
