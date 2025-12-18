-- Add Lua transform mode support to transfer configs
ALTER TABLE transfer_configs ADD COLUMN mode TEXT NOT NULL DEFAULT 'declarative';
ALTER TABLE transfer_configs ADD COLUMN lua_script TEXT;
ALTER TABLE transfer_configs ADD COLUMN lua_script_path TEXT;
