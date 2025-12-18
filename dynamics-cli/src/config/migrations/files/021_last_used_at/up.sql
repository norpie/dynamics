-- Add last_used_at column to track when configs were last accessed
ALTER TABLE transfer_configs ADD COLUMN last_used_at TEXT;

-- Create index for sorting by last used
CREATE INDEX IF NOT EXISTS idx_transfer_configs_last_used ON transfer_configs(last_used_at DESC);
