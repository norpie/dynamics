-- Remove resolver configurations table

DROP INDEX IF EXISTS idx_transfer_resolvers_config;
DROP TABLE IF EXISTS transfer_resolvers;
