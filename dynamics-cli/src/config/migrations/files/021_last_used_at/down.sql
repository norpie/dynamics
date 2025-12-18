-- Remove last_used_at column
-- Note: SQLite doesn't support DROP COLUMN directly in older versions
-- This creates a new table without the column and copies data

DROP INDEX IF EXISTS idx_transfer_configs_last_used;

CREATE TABLE transfer_configs_backup AS SELECT 
    id, name, source_env, target_env, mode, lua_script, lua_script_path, created_at, updated_at
FROM transfer_configs;

DROP TABLE transfer_configs;

CREATE TABLE transfer_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    source_env TEXT NOT NULL,
    target_env TEXT NOT NULL,
    mode TEXT NOT NULL DEFAULT 'declarative',
    lua_script TEXT,
    lua_script_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO transfer_configs (id, name, source_env, target_env, mode, lua_script, lua_script_path, created_at, updated_at)
SELECT id, name, source_env, target_env, mode, lua_script, lua_script_path, created_at, updated_at
FROM transfer_configs_backup;

DROP TABLE transfer_configs_backup;
