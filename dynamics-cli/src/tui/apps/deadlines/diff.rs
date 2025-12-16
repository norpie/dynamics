//! Deadline matching and diffing utilities for edit/update support.
//!
//! This module handles:
//! - Matching transformed Excel rows against existing Dynamics 365 deadlines
//! - Diffing fields to detect changes
//! - Diffing N:N associations to determine add/remove operations

use std::collections::HashSet;

use super::models::{
    DeadlineLookupKey, DeadlineLookupMap, DeadlineMode, ExistingAssociations,
    ExistingDeadline, TransformedDeadline,
};

/// Result of matching and diffing a single record
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// The determined mode for this record
    pub mode: DeadlineMode,
    /// If matched, the existing deadline GUID
    pub existing_guid: Option<String>,
    /// If matched, the existing field values (for showing diff in UI)
    pub existing_fields: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// If matched, the existing associations (for building update operations)
    pub existing_associations: Option<ExistingAssociations>,
}

/// Association diff result - what needs to be added/removed
#[derive(Clone, Debug, Default)]
pub struct AssociationDiff {
    /// Support IDs to add
    pub support_to_add: HashSet<String>,
    /// Support IDs to remove
    pub support_to_remove: HashSet<String>,
    /// Category IDs to add
    pub category_to_add: HashSet<String>,
    /// Category IDs to remove
    pub category_to_remove: HashSet<String>,
    /// Length IDs to add (CGK only)
    pub length_to_add: HashSet<String>,
    /// Length IDs to remove (CGK only)
    pub length_to_remove: HashSet<String>,
    /// Flemish share IDs to add
    pub flemishshare_to_add: HashSet<String>,
    /// Flemish share IDs to remove
    pub flemishshare_to_remove: HashSet<String>,
    /// Subcategory IDs to add (NRQ only)
    pub subcategory_to_add: HashSet<String>,
    /// Subcategory IDs to remove (NRQ only)
    pub subcategory_to_remove: HashSet<String>,
}

impl AssociationDiff {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.support_to_add.is_empty()
            || !self.support_to_remove.is_empty()
            || !self.category_to_add.is_empty()
            || !self.category_to_remove.is_empty()
            || !self.length_to_add.is_empty()
            || !self.length_to_remove.is_empty()
            || !self.flemishshare_to_add.is_empty()
            || !self.flemishshare_to_remove.is_empty()
            || !self.subcategory_to_add.is_empty()
            || !self.subcategory_to_remove.is_empty()
    }
}

/// Match a transformed deadline against the existing deadlines lookup map.
///
/// Returns the appropriate mode and existing record info if matched.
pub fn match_deadline(
    transformed: &TransformedDeadline,
    lookup_map: &DeadlineLookupMap,
    entity_type: &str,
) -> MatchResult {
    // Get the deadline name and date for lookup
    let name = match transformed.get_deadline_name(entity_type) {
        Some(n) => n.trim().to_lowercase(),
        None => {
            // No name - can't match, treat as create
            log::debug!(
                "Row {}: No deadline name found, treating as Create",
                transformed.source_row
            );
            return MatchResult {
                mode: DeadlineMode::Create,
                existing_guid: None,
                existing_fields: None,
                existing_associations: None,
            };
        }
    };

    let date = match transformed.deadline_date {
        Some(d) => d,
        None => {
            // No date - can't match, treat as create
            log::debug!(
                "Row {}: No deadline date found, treating as Create",
                transformed.source_row
            );
            return MatchResult {
                mode: DeadlineMode::Create,
                existing_guid: None,
                existing_fields: None,
                existing_associations: None,
            };
        }
    };

    // Build lookup key
    let key: DeadlineLookupKey = (name.clone(), date);

    // Look up in map
    match lookup_map.get(&key) {
        None => {
            // No match - create new
            log::debug!(
                "Row {}: No existing deadline found for ({}, {}), mode=Create",
                transformed.source_row,
                name,
                date
            );
            MatchResult {
                mode: DeadlineMode::Create,
                existing_guid: None,
                existing_fields: None,
                existing_associations: None,
            }
        }
        Some(existing) => {
            // Found a match - diff to determine if Update or Unchanged
            log::debug!(
                "Row {}: Found existing deadline {} for ({}, {})",
                transformed.source_row,
                existing.id,
                name,
                date
            );

            let has_field_changes = diff_fields(transformed, existing, entity_type);
            let association_diff = diff_associations(transformed, &existing.associations, entity_type);
            let has_association_changes = association_diff.has_changes();

            let mode = if has_field_changes || has_association_changes {
                log::debug!(
                    "Row {}: Changes detected (fields={}, associations={}), mode=Update",
                    transformed.source_row,
                    has_field_changes,
                    has_association_changes
                );
                DeadlineMode::Update
            } else {
                log::debug!(
                    "Row {}: No changes detected, mode=Unchanged",
                    transformed.source_row
                );
                DeadlineMode::Unchanged
            };

            MatchResult {
                mode,
                existing_guid: Some(existing.id.clone()),
                existing_fields: Some(existing.fields.clone()),
                existing_associations: Some(existing.associations.clone()),
            }
        }
    }
}

/// Match all transformed records against existing deadlines and update their modes.
pub fn match_all_deadlines(
    records: &mut [TransformedDeadline],
    lookup_map: &DeadlineLookupMap,
    entity_type: &str,
) {
    for record in records.iter_mut() {
        let result = match_deadline(record, lookup_map, entity_type);
        record.mode = result.mode;
        record.existing_guid = result.existing_guid;
        record.existing_fields = result.existing_fields;
        record.existing_associations = result.existing_associations;
    }

    // Log summary
    let create_count = records.iter().filter(|r| r.is_create()).count();
    let update_count = records.iter().filter(|r| r.is_update()).count();
    let unchanged_count = records.iter().filter(|r| r.is_unchanged()).count();
    let error_count = records.iter().filter(|r| r.is_error()).count();

    log::info!(
        "Matching complete: {} create, {} update, {} unchanged, {} error",
        create_count,
        update_count,
        unchanged_count,
        error_count
    );
}

/// Diff fields between transformed record and existing deadline.
///
/// Returns true if there are any field changes (excluding non-editable fields).
fn diff_fields(
    transformed: &TransformedDeadline,
    existing: &ExistingDeadline,
    entity_type: &str,
) -> bool {
    let is_cgk = entity_type == "cgk_deadline";

    // Non-editable fields that we skip in diff
    let non_editable: HashSet<&str> = if is_cgk {
        ["cgk_deadlinename", "cgk_date", "cgk_datumcommissievergadering"]
            .iter()
            .copied()
            .collect()
    } else {
        ["nrq_deadlinename", "nrq_deadlinedate", "nrq_committeemeetingdate"]
            .iter()
            .copied()
            .collect()
    };

    // Check direct fields
    for (field_name, new_value) in &transformed.direct_fields {
        if non_editable.contains(field_name.as_str()) {
            continue;
        }

        let existing_value = existing
            .fields
            .get(field_name)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if new_value != existing_value {
            log::debug!(
                "Field '{}' changed: '{}' -> '{}'",
                field_name,
                existing_value,
                new_value
            );
            return true;
        }
    }

    // Check lookup fields
    for (field_name, (new_guid, _target_entity)) in &transformed.lookup_fields {
        if non_editable.contains(field_name.as_str()) {
            continue;
        }

        // Lookup fields in existing data are stored as _fieldname_value
        let lookup_value_field = format!("_{}_value", field_name);
        let existing_guid = existing
            .fields
            .get(&lookup_value_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Compare GUIDs (case-insensitive)
        if !new_guid.eq_ignore_ascii_case(existing_guid) {
            log::debug!(
                "Lookup '{}' changed: '{}' -> '{}'",
                field_name,
                existing_guid,
                new_guid
            );
            return true;
        }
    }

    // Check picklist fields
    for (field_name, new_value) in &transformed.picklist_fields {
        let existing_value = existing
            .fields
            .get(field_name)
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);

        if existing_value != Some(*new_value) {
            log::debug!(
                "Picklist '{}' changed: {:?} -> {}",
                field_name,
                existing_value,
                new_value
            );
            return true;
        }
    }

    // Check boolean fields
    for (field_name, new_value) in &transformed.boolean_fields {
        let existing_value = existing.fields.get(field_name).and_then(|v| v.as_bool());

        if existing_value != Some(*new_value) {
            log::debug!(
                "Boolean '{}' changed: {:?} -> {}",
                field_name,
                existing_value,
                new_value
            );
            return true;
        }
    }

    // Check deadline time (if it changed)
    // Note: We compare times but the date is non-editable
    // Time is typically embedded in the date field, so this needs careful handling
    // For now, skip time comparison as it's complex and may not be needed

    false
}

/// Diff associations between Excel-derived IDs and existing associations.
///
/// Returns what needs to be added and removed for each relationship.
pub fn diff_associations(
    transformed: &TransformedDeadline,
    existing: &ExistingAssociations,
    entity_type: &str,
) -> AssociationDiff {
    let mut diff = AssociationDiff::default();
    let is_cgk = entity_type == "cgk_deadline";

    // Get relationship name mappings
    let (support_rel, category_rel, length_rel, flemishshare_rel, subcategory_rel) = if is_cgk {
        (
            "cgk_deadline_cgk_support",
            "cgk_deadline_cgk_category",
            "cgk_deadline_cgk_length",
            "cgk_deadline_cgk_flemishshare",
            "", // CGK has no subcategory
        )
    } else {
        (
            "", // NRQ support uses custom junction, handled separately
            "nrq_deadline_nrq_category",
            "", // NRQ has no length
            "nrq_deadline_nrq_flemishshare",
            "nrq_deadline_nrq_subcategory",
        )
    };

    // Diff support (for CGK only - NRQ uses custom junction)
    if is_cgk {
        let excel_support_ids: HashSet<String> = transformed
            .checkbox_relationships
            .get(support_rel)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        diff.support_to_add = excel_support_ids
            .difference(&existing.support_ids)
            .cloned()
            .collect();
        diff.support_to_remove = existing
            .support_ids
            .difference(&excel_support_ids)
            .cloned()
            .collect();
    } else {
        // For NRQ, diff support via custom junction records
        let excel_support_ids: HashSet<String> = transformed
            .custom_junction_records
            .iter()
            .filter(|r| r.junction_entity == "nrq_deadlinesupport")
            .map(|r| r.related_id.clone())
            .collect();

        diff.support_to_add = excel_support_ids
            .difference(&existing.support_ids)
            .cloned()
            .collect();
        diff.support_to_remove = existing
            .support_ids
            .difference(&excel_support_ids)
            .cloned()
            .collect();
    }

    // Diff category
    if !category_rel.is_empty() {
        let excel_category_ids: HashSet<String> = transformed
            .checkbox_relationships
            .get(category_rel)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        diff.category_to_add = excel_category_ids
            .difference(&existing.category_ids)
            .cloned()
            .collect();
        diff.category_to_remove = existing
            .category_ids
            .difference(&excel_category_ids)
            .cloned()
            .collect();
    }

    // Diff length (CGK only)
    if !length_rel.is_empty() {
        let excel_length_ids: HashSet<String> = transformed
            .checkbox_relationships
            .get(length_rel)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        diff.length_to_add = excel_length_ids
            .difference(&existing.length_ids)
            .cloned()
            .collect();
        diff.length_to_remove = existing
            .length_ids
            .difference(&excel_length_ids)
            .cloned()
            .collect();
    }

    // Diff flemishshare
    if !flemishshare_rel.is_empty() {
        let excel_flemishshare_ids: HashSet<String> = transformed
            .checkbox_relationships
            .get(flemishshare_rel)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        diff.flemishshare_to_add = excel_flemishshare_ids
            .difference(&existing.flemishshare_ids)
            .cloned()
            .collect();
        diff.flemishshare_to_remove = existing
            .flemishshare_ids
            .difference(&excel_flemishshare_ids)
            .cloned()
            .collect();
    }

    // Diff subcategory (NRQ only)
    if !subcategory_rel.is_empty() {
        let excel_subcategory_ids: HashSet<String> = transformed
            .checkbox_relationships
            .get(subcategory_rel)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        diff.subcategory_to_add = excel_subcategory_ids
            .difference(&existing.subcategory_ids)
            .cloned()
            .collect();
        diff.subcategory_to_remove = existing
            .subcategory_ids
            .difference(&excel_subcategory_ids)
            .cloned()
            .collect();
    }

    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn make_transformed(name: &str, date: NaiveDate, entity_type: &str) -> TransformedDeadline {
        let mut transformed = TransformedDeadline::new(1);
        let name_field = if entity_type == "cgk_deadline" {
            "cgk_deadlinename"
        } else {
            "nrq_deadlinename"
        };
        transformed.direct_fields.insert(name_field.to_string(), name.to_string());
        transformed.deadline_date = Some(date);
        transformed
    }

    fn make_existing(id: &str, name: &str, date: NaiveDate) -> ExistingDeadline {
        ExistingDeadline {
            id: id.to_string(),
            name: name.to_string(),
            date,
            fields: HashMap::new(),
            associations: ExistingAssociations::default(),
        }
    }

    #[test]
    fn test_match_not_found_returns_create() {
        let transformed = make_transformed("New Deadline", NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(), "cgk_deadline");
        let lookup_map = DeadlineLookupMap::new();

        let result = match_deadline(&transformed, &lookup_map, "cgk_deadline");

        assert_eq!(result.mode, DeadlineMode::Create);
        assert!(result.existing_guid.is_none());
    }

    #[test]
    fn test_match_found_no_changes_returns_unchanged() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let transformed = make_transformed("Test Deadline", date, "cgk_deadline");
        let existing = make_existing("guid-123", "Test Deadline", date);

        let mut lookup_map = DeadlineLookupMap::new();
        lookup_map.insert(("test deadline".to_string(), date), existing);

        let result = match_deadline(&transformed, &lookup_map, "cgk_deadline");

        assert_eq!(result.mode, DeadlineMode::Unchanged);
        assert_eq!(result.existing_guid, Some("guid-123".to_string()));
    }

    #[test]
    fn test_association_diff_detects_additions() {
        let mut transformed = TransformedDeadline::new(1);
        transformed.checkbox_relationships.insert(
            "cgk_deadline_cgk_support".to_string(),
            vec!["support-1".to_string(), "support-2".to_string()],
        );

        let existing = ExistingAssociations {
            support_ids: ["support-1".to_string()].into_iter().collect(),
            ..Default::default()
        };

        let diff = diff_associations(&transformed, &existing, "cgk_deadline");

        assert!(diff.support_to_add.contains("support-2"));
        assert!(diff.support_to_remove.is_empty());
    }

    #[test]
    fn test_association_diff_detects_removals() {
        let transformed = TransformedDeadline::new(1);
        // No checkbox_relationships means empty Excel associations

        let existing = ExistingAssociations {
            support_ids: ["support-1".to_string()].into_iter().collect(),
            ..Default::default()
        };

        let diff = diff_associations(&transformed, &existing, "cgk_deadline");

        assert!(diff.support_to_add.is_empty());
        assert!(diff.support_to_remove.contains("support-1"));
    }
}
