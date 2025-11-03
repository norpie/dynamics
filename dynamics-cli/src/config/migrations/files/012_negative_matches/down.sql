-- Revert negative_matches table

DROP INDEX IF EXISTS idx_negative_matches_lookup;
DROP TABLE IF EXISTS negative_matches;
