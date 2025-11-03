-- Add negative_matches table to block specific fields from prefix matching
-- Negative matches override automatic prefix transformations for specific source fields

CREATE TABLE negative_matches (
    id INTEGER PRIMARY KEY,
    source_entity TEXT NOT NULL,
    target_entity TEXT NOT NULL,
    source_field TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_entity, target_entity, source_field)
);

-- Index for efficient lookup during matching
CREATE INDEX idx_negative_matches_lookup
ON negative_matches(source_entity, target_entity, source_field);
