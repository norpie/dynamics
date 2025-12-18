-- SQLite doesn't support DROP COLUMN directly, so we need to recreate the table
-- This is a destructive migration - lua_script and lua_script_path data will be lost

-- Create a backup of the old table
CREATE TABLE transfer_configs_backup AS SELECT id, name, source_env, target_env, created_at, updated_at FROM transfer_configs;

-- Drop the old table
DROP TABLE transfer_configs;

-- Recreate without new columns
CREATE TABLE transfer_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    source_env TEXT NOT NULL,
    target_env TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Restore data
INSERT INTO transfer_configs (id, name, source_env, target_env, created_at, updated_at)
SELECT id, name, source_env, target_env, created_at, updated_at FROM transfer_configs_backup;

-- Drop backup
DROP TABLE transfer_configs_backup;
