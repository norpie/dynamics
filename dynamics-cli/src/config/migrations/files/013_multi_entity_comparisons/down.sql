-- Rollback multi-entity comparison support

-- Remove the JSON array columns
ALTER TABLE comparisons DROP COLUMN source_entities;
ALTER TABLE comparisons DROP COLUMN target_entities;

-- Note: Legacy columns (source_entity, target_entity) remain intact
