-- Add support for multiple source and target entities per comparison

-- Add JSON array columns for entity lists
ALTER TABLE comparisons ADD COLUMN source_entities TEXT;  -- JSON: ["contact", "account"]
ALTER TABLE comparisons ADD COLUMN target_entities TEXT;  -- JSON: ["lead", "customer"]

-- Populate from legacy columns for backwards compatibility
-- Wrap existing single entities in JSON arrays
UPDATE comparisons
SET source_entities = '["' || source_entity || '"]',
    target_entities = '["' || target_entity || '"]'
WHERE source_entities IS NULL OR target_entities IS NULL;
