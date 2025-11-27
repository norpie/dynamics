//! Schema diff logic for comparing entity schemas between environments
//!
//! This module provides functions to:
//! - Compare field schemas between origin and target entities
//! - Categorize fields as matching, origin-only, target-only, or type-mismatch
//! - Filter out system fields that should be skipped
//! - Generate a complete schema diff report

use std::collections::HashMap;
use serde_json::Value;

use crate::api::metadata::{FieldMetadata, FieldType, EntityMetadata};
use crate::tui::apps::sync::types::{
    EntitySchemaDiff, FieldDiffEntry, FieldSyncStatus, is_system_field,
};

/// Compare two entity schemas and produce a diff
///
/// # Arguments
/// * `entity_name` - The logical name of the entity being compared
/// * `origin_fields` - Fields from the origin environment
/// * `target_fields` - Fields from the target environment
/// * `origin_raw` - Optional raw API response for origin fields (for CreateAttribute)
///
/// # Returns
/// An `EntitySchemaDiff` containing categorized field differences
pub fn compare_schemas(
    entity_name: &str,
    origin_fields: &[FieldMetadata],
    target_fields: &[FieldMetadata],
    origin_raw: Option<&HashMap<String, Value>>,
) -> EntitySchemaDiff {
    // Build lookup maps for efficient comparison
    let origin_map: HashMap<&str, &FieldMetadata> = origin_fields
        .iter()
        .map(|f| (f.logical_name.as_str(), f))
        .collect();

    let target_map: HashMap<&str, &FieldMetadata> = target_fields
        .iter()
        .map(|f| (f.logical_name.as_str(), f))
        .collect();

    let mut diff = EntitySchemaDiff {
        entity_name: entity_name.to_string(),
        ..Default::default()
    };

    // Check all origin fields
    for origin_field in origin_fields {
        let field_name = &origin_field.logical_name;
        let is_system = is_system_field(field_name);

        // Get raw metadata for this field if available
        let raw_metadata = origin_raw.and_then(|m| m.get(field_name).cloned());

        let entry = FieldDiffEntry {
            logical_name: field_name.clone(),
            display_name: origin_field.display_name.clone(),
            field_type: format_field_type(&origin_field.field_type),
            status: FieldSyncStatus::InBoth, // Will be updated below
            is_system_field: is_system,
            origin_metadata: raw_metadata,
        };

        if let Some(target_field) = target_map.get(field_name.as_str()) {
            // Field exists in both - check if types match
            if fields_match(origin_field, target_field) {
                let mut entry = entry;
                entry.status = FieldSyncStatus::InBoth;
                diff.fields_in_both.push(entry);
            } else {
                let mut entry = entry;
                entry.status = FieldSyncStatus::TypeMismatch {
                    origin_type: format_field_type(&origin_field.field_type),
                    target_type: format_field_type(&target_field.field_type),
                };
                diff.fields_type_mismatch.push(entry);
            }
        } else {
            // Field only in origin - will be added to target
            let mut entry = entry;
            entry.status = FieldSyncStatus::OriginOnly;
            diff.fields_to_add.push(entry);
        }
    }

    // Check for fields only in target
    for target_field in target_fields {
        let field_name = &target_field.logical_name;

        if !origin_map.contains_key(field_name.as_str()) {
            let is_system = is_system_field(field_name);

            let entry = FieldDiffEntry {
                logical_name: field_name.clone(),
                display_name: target_field.display_name.clone(),
                field_type: format_field_type(&target_field.field_type),
                status: FieldSyncStatus::TargetOnly,
                is_system_field: is_system,
                origin_metadata: None,
            };
            diff.fields_target_only.push(entry);
        }
    }

    // Sort all lists by field name for consistent display
    diff.fields_in_both.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    diff.fields_to_add.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    diff.fields_target_only.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    diff.fields_type_mismatch.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));

    diff
}

/// Check if two fields are compatible (same type)
fn fields_match(origin: &FieldMetadata, target: &FieldMetadata) -> bool {
    // For now, just compare field types
    // Could be extended to check max_length, required, etc.
    origin.field_type == target.field_type
}

/// Format a field type for display
pub fn format_field_type(field_type: &FieldType) -> String {
    match field_type {
        FieldType::String => "String".to_string(),
        FieldType::Integer => "Integer".to_string(),
        FieldType::Decimal => "Decimal".to_string(),
        FieldType::Boolean => "Boolean".to_string(),
        FieldType::DateTime => "DateTime".to_string(),
        FieldType::Lookup => "Lookup".to_string(),
        FieldType::OptionSet => "OptionSet".to_string(),
        FieldType::Money => "Money".to_string(),
        FieldType::Memo => "Memo".to_string(),
        FieldType::UniqueIdentifier => "UniqueIdentifier".to_string(),
        FieldType::Other(s) => s.clone(),
    }
}

/// Filter fields to only include non-system fields that can be synced
pub fn filter_syncable_fields(fields: &[FieldMetadata]) -> Vec<&FieldMetadata> {
    fields
        .iter()
        .filter(|f| !is_system_field(&f.logical_name))
        .collect()
}

/// Get lookup fields from a list of fields
pub fn get_lookup_fields(fields: &[FieldMetadata]) -> Vec<&FieldMetadata> {
    fields
        .iter()
        .filter(|f| matches!(f.field_type, FieldType::Lookup))
        .collect()
}

/// Extract the target entities from lookup fields
pub fn get_lookup_targets(fields: &[FieldMetadata]) -> Vec<String> {
    fields
        .iter()
        .filter(|f| matches!(f.field_type, FieldType::Lookup))
        .filter_map(|f| f.related_entity.clone())
        .collect()
}

/// Generate summary statistics for a schema diff
#[derive(Debug, Clone, Default)]
pub struct SchemaDiffStats {
    pub total_origin_fields: usize,
    pub total_target_fields: usize,
    pub fields_matching: usize,
    pub fields_to_add: usize,
    pub fields_target_only: usize,
    pub fields_type_mismatch: usize,
    pub system_fields_skipped: usize,
}

impl SchemaDiffStats {
    pub fn from_diff(diff: &EntitySchemaDiff) -> Self {
        let system_skipped = diff.fields_in_both.iter().filter(|f| f.is_system_field).count()
            + diff.fields_to_add.iter().filter(|f| f.is_system_field).count()
            + diff.fields_target_only.iter().filter(|f| f.is_system_field).count();

        Self {
            total_origin_fields: diff.fields_in_both.len() + diff.fields_to_add.len() + diff.fields_type_mismatch.len(),
            total_target_fields: diff.fields_in_both.len() + diff.fields_target_only.len() + diff.fields_type_mismatch.len(),
            fields_matching: diff.fields_in_both.len(),
            fields_to_add: diff.fields_to_add.len(),
            fields_target_only: diff.fields_target_only.len(),
            fields_type_mismatch: diff.fields_type_mismatch.len(),
            system_fields_skipped: system_skipped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_field(name: &str, field_type: FieldType) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            display_name: Some(name.to_string()),
            field_type,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: None,
        }
    }

    fn make_lookup(name: &str, target: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            display_name: Some(name.to_string()),
            field_type: FieldType::Lookup,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: Some(target.to_string()),
        }
    }

    #[test]
    fn test_compare_identical_schemas() {
        let origin = vec![
            make_field("name", FieldType::String),
            make_field("amount", FieldType::Decimal),
        ];
        let target = origin.clone();

        let diff = compare_schemas("test_entity", &origin, &target, None);

        assert_eq!(diff.fields_in_both.len(), 2);
        assert!(diff.fields_to_add.is_empty());
        assert!(diff.fields_target_only.is_empty());
        assert!(diff.fields_type_mismatch.is_empty());
    }

    #[test]
    fn test_compare_origin_only_fields() {
        let origin = vec![
            make_field("name", FieldType::String),
            make_field("new_field", FieldType::Integer),
        ];
        let target = vec![
            make_field("name", FieldType::String),
        ];

        let diff = compare_schemas("test_entity", &origin, &target, None);

        assert_eq!(diff.fields_in_both.len(), 1);
        assert_eq!(diff.fields_to_add.len(), 1);
        assert_eq!(diff.fields_to_add[0].logical_name, "new_field");
        assert!(diff.fields_target_only.is_empty());
    }

    #[test]
    fn test_compare_target_only_fields() {
        let origin = vec![
            make_field("name", FieldType::String),
        ];
        let target = vec![
            make_field("name", FieldType::String),
            make_field("old_field", FieldType::Integer),
        ];

        let diff = compare_schemas("test_entity", &origin, &target, None);

        assert_eq!(diff.fields_in_both.len(), 1);
        assert!(diff.fields_to_add.is_empty());
        assert_eq!(diff.fields_target_only.len(), 1);
        assert_eq!(diff.fields_target_only[0].logical_name, "old_field");
    }

    #[test]
    fn test_compare_type_mismatch() {
        let origin = vec![
            make_field("amount", FieldType::Decimal),
        ];
        let target = vec![
            make_field("amount", FieldType::Integer),
        ];

        let diff = compare_schemas("test_entity", &origin, &target, None);

        assert!(diff.fields_in_both.is_empty());
        assert!(diff.fields_to_add.is_empty());
        assert!(diff.fields_target_only.is_empty());
        assert_eq!(diff.fields_type_mismatch.len(), 1);
    }

    #[test]
    fn test_system_fields_marked() {
        let origin = vec![
            make_field("name", FieldType::String),
            make_field("createdby", FieldType::Lookup),
            make_field("modifiedon", FieldType::DateTime),
        ];
        let target = origin.clone();

        let diff = compare_schemas("test_entity", &origin, &target, None);

        let system_count = diff.fields_in_both.iter().filter(|f| f.is_system_field).count();
        assert_eq!(system_count, 2); // createdby and modifiedon
    }

    #[test]
    fn test_get_lookup_targets() {
        let fields = vec![
            make_field("name", FieldType::String),
            make_lookup("accountid", "account"),
            make_lookup("contactid", "contact"),
        ];

        let targets = get_lookup_targets(&fields);

        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&"account".to_string()));
        assert!(targets.contains(&"contact".to_string()));
    }
}
