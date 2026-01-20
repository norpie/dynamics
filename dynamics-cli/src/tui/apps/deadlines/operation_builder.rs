//! Convert TransformedDeadline records to Dynamics 365 API Operations
//!
//! This module handles the conversion of validated and transformed deadline records
//! into executable API operations, including:
//! - Main entity creation with resolved lookups
//! - Junction entity creation for N:N relationships (using Content-ID references)
//! - DateTime timezone conversion (Brussels → UTC)
//! - Proper @odata.bind formatting for lookups

use super::diff::{AssociationDiff, diff_associations};
use super::field_mappings::get_constant_fields;
use super::models::{DeadlineMode, ExistingAssociations, TransformedDeadline};
use crate::api::operations::Operation;
use crate::api::pluralization::pluralize_entity_name;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

impl TransformedDeadline {
    /// Convert this TransformedDeadline to a list of Operations ready for batch execution
    ///
    /// Returns a Vec with a single operation that creates the deadline
    pub fn to_operations(&self, entity_type: &str) -> Vec<Operation> {
        let entity_set = pluralize_entity_name(entity_type);
        let payload = self.build_create_payload(entity_type);

        vec![Operation::Create {
            entity: entity_set,
            data: payload,
        }]
    }

    /// Build the JSON payload for creating the main deadline entity
    fn build_create_payload(&self, entity_type: &str) -> Value {
        let mut payload = json!({});
        let name_field = if entity_type == "cgk_deadline" {
            "cgk_deadlinename"
        } else {
            "nrq_deadlinename"
        };
        let date_field = if entity_type == "cgk_deadline" {
            "cgk_date"
        } else {
            "nrq_deadlinedate"
        };

        // Warn if required fields are missing
        if !self.direct_fields.contains_key(name_field) {
            log::warn!(
                "Create payload missing required field: {} (row {})",
                name_field,
                self.source_row
            );
        }
        if self.deadline_date.is_none() {
            log::warn!(
                "Create payload missing required field: {} (row {})",
                date_field,
                self.source_row
            );
        }

        // 0. Constant fields (entity-specific defaults)
        for (field, value) in get_constant_fields(entity_type) {
            payload[field] = value;
        }

        // 1. Direct fields (name, info, etc.)
        for (field, value) in &self.direct_fields {
            payload[field] = json!(value);
        }

        // NRQ: Generate nrq_name field (formatted as "Deadline - Date Time")
        if entity_type == "nrq_deadline" {
            if let Some(deadline_name) = self.direct_fields.get("nrq_deadlinename") {
                let mut name_parts = vec![deadline_name.clone()];

                // Add formatted date/time if available
                if let Some(date) = self.deadline_date {
                    let date_str = date.format("%d/%m/%Y").to_string();
                    let time_str = if let Some(time) = self.deadline_time {
                        time.format("%H:%M").to_string()
                    } else {
                        "12:00".to_string()
                    };
                    name_parts.push(format!("{} {}", date_str, time_str));
                }

                payload["nrq_name"] = json!(name_parts.join(" - "));
            }
        }

        // 1a. Picklist fields (integer values)
        for (field, value) in &self.picklist_fields {
            payload[field] = json!(value);
        }

        // 1b. Boolean fields
        for (field, value) in &self.boolean_fields {
            payload[field] = json!(value);
        }

        // 1c. Generate cgk_name field (formatted as "Deadline - Date Time")
        if entity_type == "cgk_deadline" {
            if let Some(deadline_name) = self.direct_fields.get("cgk_deadlinename") {
                let mut name_parts = vec![deadline_name.clone()];

                // Add formatted date/time if available
                if let Some(date) = self.deadline_date {
                    let date_str = date.format("%d/%m/%Y").to_string();
                    let time_str = if let Some(time) = self.deadline_time {
                        time.format("%H:%M").to_string()
                    } else {
                        "12:00".to_string()
                    };
                    name_parts.push(format!("{} {}", date_str, time_str));
                }

                payload["cgk_name"] = json!(name_parts.join(" - "));
            }
        }

        // 2. Lookup fields (@odata.bind format)
        for (field, (id, target_entity)) in &self.lookup_fields {
            let bind_field = format!("{}@odata.bind", field);
            let entity_set = pluralize_entity_name(target_entity);
            payload[bind_field] = json!(format!("/{}({})", entity_set, id));
        }

        // 3. Deadline date/time (combined if both present)
        if let Some(date) = self.deadline_date {
            let date_field = if entity_type == "cgk_deadline" {
                "cgk_date"
            } else {
                "nrq_deadlinedate"
            };

            if let Some(time) = self.deadline_time {
                // Combine date + time, convert Brussels → UTC
                if let Ok(datetime_str) = combine_brussels_datetime_to_iso(date, Some(time)) {
                    payload[date_field] = json!(datetime_str);
                }
            } else {
                // Date-only (no time) - use 12:00 Brussels as default
                if let Ok(datetime_str) = combine_brussels_datetime_to_iso(date, None) {
                    payload[date_field] = json!(datetime_str);
                }
            }
        }

        // 4. Commission date/time (CGK and NRQ both support this)
        if let Some(date) = self.commission_date {
            let commission_field = if entity_type == "cgk_deadline" {
                "cgk_datumcommissievergadering"
            } else {
                "nrq_committeemeetingdate"
            };

            if let Some(time) = self.commission_time {
                // Combine date + time, convert Brussels → UTC
                if let Ok(datetime_str) = combine_brussels_datetime_to_iso(date, Some(time)) {
                    payload[commission_field] = json!(datetime_str);
                }
            } else {
                // Date-only (no time) - use 12:00 Brussels as default
                if let Ok(datetime_str) = combine_brussels_datetime_to_iso(date, None) {
                    payload[commission_field] = json!(datetime_str);
                }
            }
        }

        // N:N relationships are handled separately via AssociateRef operations after deadline creation
        // (see deadlines_inspection_app.rs)

        payload
    }

    /// Convert this TransformedDeadline to an Update operation (for existing records)
    ///
    /// Only includes editable fields that have changed.
    pub fn to_update_operations(&self, entity_type: &str) -> Vec<Operation> {
        let entity_set = pluralize_entity_name(entity_type);

        // Must have existing_guid to update
        let entity_guid = match &self.existing_guid {
            Some(guid) => guid.clone(),
            None => {
                log::error!("Cannot create Update operation without existing_guid");
                return vec![];
            }
        };

        let payload = self.build_update_payload(entity_type);

        // If payload is empty (no changes), return empty vec
        if payload.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            log::debug!("No field changes for update - skipping PATCH operation");
            return vec![];
        }

        vec![Operation::Update {
            entity: entity_set,
            id: entity_guid,
            data: payload,
        }]
    }

    /// Build the JSON payload for updating an existing deadline entity
    ///
    /// NOTE: Business rules require deadline name and date to always be included
    fn build_update_payload(&self, entity_type: &str) -> Value {
        let mut payload = json!({});
        let is_cgk = entity_type == "cgk_deadline";

        // Fields that shouldn't be modified but some need to be included for validation
        // Commission date is still excluded as it's truly locked
        let excluded: HashSet<&str> = if is_cgk {
            ["cgk_datumcommissievergadering", "cgk_name"]
                .iter()
                .copied()
                .collect()
        } else {
            ["nrq_committeemeetingdate", "nrq_name"]
                .iter()
                .copied()
                .collect()
        };

        // Always include deadline name and date (required by business rules)
        let (name_field, date_field) = if is_cgk {
            ("cgk_deadlinename", "cgk_date")
        } else {
            ("nrq_deadlinename", "nrq_deadlinedate")
        };

        if let Some(name) = self.direct_fields.get(name_field) {
            payload[name_field] = json!(name);
        } else {
            log::warn!("Update payload missing required field: {}", name_field);
        }

        if let Some(date) = self.deadline_date {
            // Use same timezone conversion as create payload
            if let Ok(datetime_str) = combine_brussels_datetime_to_iso(date, self.deadline_time) {
                payload[date_field] = json!(datetime_str);
            }
        } else {
            log::warn!("Update payload missing required field: {}", date_field);
        }

        // 1. Direct fields (excluding already added and excluded)
        for (field, value) in &self.direct_fields {
            if field != name_field && !excluded.contains(field.as_str()) {
                payload[field] = json!(value);
            }
        }

        // 2. Picklist fields
        for (field, value) in &self.picklist_fields {
            if !excluded.contains(field.as_str()) {
                payload[field] = json!(value);
            }
        }

        // 3. Boolean fields
        for (field, value) in &self.boolean_fields {
            if !excluded.contains(field.as_str()) {
                payload[field] = json!(value);
            }
        }

        // 4. Lookup fields (@odata.bind format)
        for (field, (id, target_entity)) in &self.lookup_fields {
            if !excluded.contains(field.as_str()) {
                let bind_field = format!("{}@odata.bind", field);
                let entity_set = pluralize_entity_name(target_entity);
                payload[bind_field] = json!(format!("/{}({})", entity_set, id));
            }
        }

        // N:N relationships are handled separately via Associate/Disassociate operations

        payload
    }
}

/// Get the junction entity name for a given entity type and relationship
///
/// # CGK Pattern (varies by entity!)
/// - cgk_deadline_cgk_support → cgk_cgk_deadline_cgk_support
/// - cgk_deadline_cgk_category → cgk_cgk_deadline_cgk_category
/// - cgk_deadline_cgk_length → cgk_cgk_deadline_cgk_length
/// - cgk_deadline_cgk_flemishshare → cgk_cgk_flemishshare_cgk_deadline (REVERSED ORDER!)
///
/// # NRQ Pattern (different!)
/// - nrq_deadline_nrq_support → nrq_Deadline_nrq_Support_nrq_Support
/// - nrq_deadline_nrq_category → nrq_Deadline_nrq_Category_nrq_Category
/// - nrq_deadline_nrq_flemishshare → nrq_Deadline_nrq_Flemishshare_nrq_Flemish
pub fn get_junction_entity_name(entity_type: &str, relationship_name: &str) -> String {
    if entity_type == "cgk_deadline" {
        // CGK: Pattern varies by entity
        match relationship_name {
            "cgk_deadline_cgk_support" => "cgk_cgk_deadline_cgk_support".to_string(),
            "cgk_deadline_cgk_category" => "cgk_cgk_deadline_cgk_category".to_string(),
            "cgk_deadline_cgk_length" => "cgk_cgk_deadline_cgk_length".to_string(),
            "cgk_deadline_cgk_flemishshare" => "cgk_cgk_flemishshare_cgk_deadline".to_string(), // REVERSED ORDER!
            _ => {
                log::warn!(
                    "Unknown CGK relationship '{}', using fallback pattern",
                    relationship_name
                );
                format!("cgk_{}", relationship_name)
            }
        }
    } else if entity_type == "nrq_deadline" {
        // NRQ: Complex pattern with capitalization - nrq_Deadline_nrq_{Entity}_nrq_{FinalEntity}
        // NOTE: nrq_support is NOT here - it uses a custom junction entity (nrq_deadlinesupport)
        // and is handled separately via custom_junction_records
        match relationship_name {
            "nrq_deadline_nrq_category" => "nrq_Deadline_nrq_Category_nrq_Category".to_string(),
            "nrq_deadline_nrq_subcategory" => {
                "nrq_Deadline_nrq_Subcategory_nrq_Subcategory".to_string()
            }
            "nrq_deadline_nrq_flemishshare" => {
                "nrq_Deadline_nrq_FlemishShare_nrq_Flemish".to_string()
            }
            _ => {
                log::warn!(
                    "Unknown NRQ relationship '{}', using fallback pattern",
                    relationship_name
                );
                // Extract entity name and capitalize
                let entity = relationship_name
                    .strip_prefix("nrq_deadline_nrq_")
                    .unwrap_or(relationship_name);
                let capitalized = capitalize_first_letter(entity);
                format!("nrq_Deadline_nrq_{}_nrq_{}", capitalized, capitalized)
            }
        }
    } else {
        log::error!("Unknown entity type: {}", entity_type);
        format!("unknown_{}", relationship_name)
    }
}

/// Extract the related entity name from a relationship name
///
/// Examples:
/// - "cgk_deadline_cgk_support" → "cgk_support"
/// - "nrq_deadline_nrq_category" → "nrq_category"
pub fn extract_related_entity_from_relationship(relationship_name: &str) -> String {
    // Split on underscores and skip first 2 parts (entity_deadline_)
    let parts: Vec<&str> = relationship_name.split('_').collect();

    if parts.len() >= 4 {
        // Join everything after "xxx_deadline_"
        parts[2..].join("_")
    } else {
        log::warn!("Unexpected relationship name format: {}", relationship_name);
        relationship_name.to_string()
    }
}

/// Extract entity base name from a lookup field name
///
/// Examples:
/// - "cgk_pillarid" → "cgk_pillar"
/// - "ownerid" → "owner"
/// - "cgk_fundid" → "cgk_fund"
fn extract_entity_base_from_field(field: &str) -> String {
    field.trim_end_matches("id").to_string()
}

/// Combine Brussels local date/time and convert to UTC ISO 8601 string
///
/// Format: "YYYY-MM-DDTHH:MM:SS.000Z"
///
/// Handles DST transitions automatically using chrono-tz.
fn combine_brussels_datetime_to_iso(
    date: chrono::NaiveDate,
    time: Option<chrono::NaiveTime>,
) -> Result<String, String> {
    use chrono::{LocalResult, TimeZone, Utc};
    use chrono_tz::Europe::Brussels;

    // Use 12:00 as default if no time provided
    let local_time = time.unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());

    let brussels_naive = date.and_time(local_time);

    // Convert Brussels → UTC
    match Brussels.from_local_datetime(&brussels_naive) {
        LocalResult::Single(brussels_dt) => {
            let utc_dt = brussels_dt.with_timezone(&Utc);
            Ok(utc_dt.format("%Y-%m-%dT%H:%M:%S.%3fZ").to_string())
        }
        LocalResult::Ambiguous(earlier, _later) => {
            // Fall back transition: use earlier occurrence
            let utc_dt = earlier.with_timezone(&Utc);
            Ok(utc_dt.format("%Y-%m-%dT%H:%M:%S.%3fZ").to_string())
        }
        LocalResult::None => {
            // Spring forward gap
            Err(format!(
                "Invalid Brussels time (DST gap): {}",
                brussels_naive
            ))
        }
    }
}

/// Capitalize the first letter of a string
fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// ============================================================================
// Update/Edit Support Operations
// ============================================================================

/// Build DisassociateRef operations for removed N:N relationships
///
/// For each association that exists in Dynamics but not in Excel, generate a DELETE $ref operation.
pub fn build_disassociate_operations(
    entity_guid: &str,
    entity_type: &str,
    association_diff: &AssociationDiff,
) -> Vec<Operation> {
    let mut operations = Vec::new();
    let entity_set = pluralize_entity_name(entity_type);
    let is_cgk = entity_type == "cgk_deadline";

    // Support disassociations (CGK only - NRQ uses custom junction)
    if is_cgk {
        let nav_prop = get_junction_entity_name(entity_type, "cgk_deadline_cgk_support");
        for support_id in &association_diff.support_to_remove {
            operations.push(Operation::DisassociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: nav_prop.clone(),
                target_id: support_id.clone(),
            });
        }
    }

    // Category disassociations
    let category_rel = if is_cgk {
        "cgk_deadline_cgk_category"
    } else {
        "nrq_deadline_nrq_category"
    };
    let nav_prop = get_junction_entity_name(entity_type, category_rel);
    for category_id in &association_diff.category_to_remove {
        operations.push(Operation::DisassociateRef {
            entity: entity_set.clone(),
            entity_ref: entity_guid.to_string(),
            navigation_property: nav_prop.clone(),
            target_id: category_id.clone(),
        });
    }

    // Length disassociations (CGK only)
    if is_cgk {
        let nav_prop = get_junction_entity_name(entity_type, "cgk_deadline_cgk_length");
        for length_id in &association_diff.length_to_remove {
            operations.push(Operation::DisassociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: nav_prop.clone(),
                target_id: length_id.clone(),
            });
        }
    }

    // Flemishshare disassociations
    let flemishshare_rel = if is_cgk {
        "cgk_deadline_cgk_flemishshare"
    } else {
        "nrq_deadline_nrq_flemishshare"
    };
    let nav_prop = get_junction_entity_name(entity_type, flemishshare_rel);
    for flemishshare_id in &association_diff.flemishshare_to_remove {
        operations.push(Operation::DisassociateRef {
            entity: entity_set.clone(),
            entity_ref: entity_guid.to_string(),
            navigation_property: nav_prop.clone(),
            target_id: flemishshare_id.clone(),
        });
    }

    // Subcategory disassociations (NRQ only)
    if !is_cgk {
        let nav_prop = get_junction_entity_name(entity_type, "nrq_deadline_nrq_subcategory");
        for subcategory_id in &association_diff.subcategory_to_remove {
            operations.push(Operation::DisassociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: nav_prop.clone(),
                target_id: subcategory_id.clone(),
            });
        }
    }

    operations
}

/// Build AssociateRef operations for added N:N relationships
///
/// For each association that exists in Excel but not in Dynamics, generate a POST $ref operation.
pub fn build_associate_operations(
    entity_guid: &str,
    entity_type: &str,
    association_diff: &AssociationDiff,
) -> Vec<Operation> {
    let mut operations = Vec::new();
    let entity_set = pluralize_entity_name(entity_type);
    let is_cgk = entity_type == "cgk_deadline";

    // Support associations (CGK only - NRQ uses custom junction)
    if is_cgk {
        let nav_prop = get_junction_entity_name(entity_type, "cgk_deadline_cgk_support");
        let related_entity_set = pluralize_entity_name("cgk_support");
        for support_id in &association_diff.support_to_add {
            operations.push(Operation::AssociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: nav_prop.clone(),
                target_ref: format!("/{}({})", related_entity_set, support_id),
            });
        }
    }

    // Category associations
    let (category_rel, category_entity) = if is_cgk {
        ("cgk_deadline_cgk_category", "cgk_category")
    } else {
        ("nrq_deadline_nrq_category", "nrq_category")
    };
    let nav_prop = get_junction_entity_name(entity_type, category_rel);
    let related_entity_set = pluralize_entity_name(category_entity);
    for category_id in &association_diff.category_to_add {
        operations.push(Operation::AssociateRef {
            entity: entity_set.clone(),
            entity_ref: entity_guid.to_string(),
            navigation_property: nav_prop.clone(),
            target_ref: format!("/{}({})", related_entity_set, category_id),
        });
    }

    // Length associations (CGK only)
    if is_cgk {
        let nav_prop = get_junction_entity_name(entity_type, "cgk_deadline_cgk_length");
        let related_entity_set = pluralize_entity_name("cgk_length");
        for length_id in &association_diff.length_to_add {
            operations.push(Operation::AssociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: nav_prop.clone(),
                target_ref: format!("/{}({})", related_entity_set, length_id),
            });
        }
    }

    // Flemishshare associations
    let (flemishshare_rel, flemishshare_entity) = if is_cgk {
        ("cgk_deadline_cgk_flemishshare", "cgk_flemishshare")
    } else {
        ("nrq_deadline_nrq_flemishshare", "nrq_flemishshare")
    };
    let nav_prop = get_junction_entity_name(entity_type, flemishshare_rel);
    let related_entity_set = pluralize_entity_name(flemishshare_entity);
    for flemishshare_id in &association_diff.flemishshare_to_add {
        operations.push(Operation::AssociateRef {
            entity: entity_set.clone(),
            entity_ref: entity_guid.to_string(),
            navigation_property: nav_prop.clone(),
            target_ref: format!("/{}({})", related_entity_set, flemishshare_id),
        });
    }

    // Subcategory associations (NRQ only)
    if !is_cgk {
        let nav_prop = get_junction_entity_name(entity_type, "nrq_deadline_nrq_subcategory");
        let related_entity_set = pluralize_entity_name("nrq_subcategory");
        for subcategory_id in &association_diff.subcategory_to_add {
            operations.push(Operation::AssociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: nav_prop.clone(),
                target_ref: format!("/{}({})", related_entity_set, subcategory_id),
            });
        }
    }

    operations
}

/// Build Delete operations for removed custom junction records (NRQ support)
///
/// For NRQ, support relationships use a custom junction entity (nrq_deadlinesupport).
/// We need to delete the junction records directly rather than disassociate.
pub fn build_delete_junction_operations(
    existing_associations: &ExistingAssociations,
    support_ids_to_remove: &HashSet<String>,
) -> Vec<Operation> {
    let mut operations = Vec::new();

    // Find junction records for the support IDs being removed
    for junction_record in &existing_associations.custom_junction_records {
        if support_ids_to_remove.contains(&junction_record.related_id) {
            operations.push(Operation::Delete {
                entity: "nrq_deadlinesupports".to_string(),
                id: junction_record.junction_id.clone(),
            });
        }
    }

    operations
}

/// Build Create operations for added custom junction records (NRQ support)
///
/// Similar to existing build_custom_junction_operations but works from diff data.
pub fn build_create_junction_operations(
    entity_guid: &str,
    support_ids_to_add: &HashSet<String>,
    custom_junction_records: &[super::models::CustomJunctionRecord],
) -> Vec<Operation> {
    let mut operations = Vec::new();

    // Find the matching CustomJunctionRecord for each support ID to add
    for support_id in support_ids_to_add {
        // Find the record with this support ID to get the related_name
        let record = custom_junction_records
            .iter()
            .find(|r| r.related_id == *support_id && r.junction_entity == "nrq_deadlinesupport");

        if let Some(record) = record {
            let mut payload = serde_json::Map::new();

            // Main entity lookup
            payload.insert(
                "nrq_DeadlineId@odata.bind".to_string(),
                json!(format!("/nrq_deadlines({})", entity_guid)),
            );

            // Related entity lookup
            payload.insert(
                "nrq_SupportId@odata.bind".to_string(),
                json!(format!("/nrq_supports({})", support_id)),
            );

            // Entity-specific fields
            payload.insert("nrq_name".to_string(), json!(record.related_name));
            payload.insert("nrq_enablehearing".to_string(), json!(false));
            payload.insert("nrq_enablereporter".to_string(), json!(true));

            operations.push(Operation::Create {
                entity: "nrq_deadlinesupports".to_string(),
                data: serde_json::Value::Object(payload),
            });
        } else {
            log::warn!(
                "No CustomJunctionRecord found for support_id {} - skipping junction create",
                support_id
            );
        }
    }

    operations
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_cgk_junction_names() {
        assert_eq!(
            get_junction_entity_name("cgk_deadline", "cgk_deadline_cgk_support"),
            "cgk_cgk_deadline_cgk_support"
        );
        assert_eq!(
            get_junction_entity_name("cgk_deadline", "cgk_deadline_cgk_category"),
            "cgk_cgk_deadline_cgk_category"
        );
    }

    #[test]
    fn test_nrq_junction_names() {
        // NOTE: nrq_support uses custom junction entity (nrq_deadlinesupport), not N:N
        assert_eq!(
            get_junction_entity_name("nrq_deadline", "nrq_deadline_nrq_category"),
            "nrq_Deadline_nrq_Category_nrq_Category"
        );
        assert_eq!(
            get_junction_entity_name("nrq_deadline", "nrq_deadline_nrq_subcategory"),
            "nrq_Deadline_nrq_Subcategory_nrq_Subcategory"
        );
        assert_eq!(
            get_junction_entity_name("nrq_deadline", "nrq_deadline_nrq_flemishshare"),
            "nrq_Deadline_nrq_FlemishShare_nrq_Flemish"
        );
    }

    #[test]
    fn test_extract_related_entity() {
        assert_eq!(
            extract_related_entity_from_relationship("cgk_deadline_cgk_support"),
            "cgk_support"
        );
        assert_eq!(
            extract_related_entity_from_relationship("nrq_deadline_nrq_subcategory"),
            "nrq_subcategory"
        );
    }

    #[test]
    fn test_extract_entity_base() {
        assert_eq!(extract_entity_base_from_field("cgk_pillarid"), "cgk_pillar");
        assert_eq!(extract_entity_base_from_field("ownerid"), "owner");
        assert_eq!(extract_entity_base_from_field("cgk_fundid"), "cgk_fund");
    }

    #[test]
    fn test_combine_brussels_datetime() {
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let time = chrono::NaiveTime::from_hms_opt(14, 30, 0).unwrap();

        let result = combine_brussels_datetime_to_iso(date, Some(time)).unwrap();

        // Brussels is UTC+1 in March (CET), so 14:30 Brussels = 13:30 UTC
        assert_eq!(result, "2025-03-15T13:30:00.000Z");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize_first_letter("support"), "Support");
        assert_eq!(capitalize_first_letter("category"), "Category");
        assert_eq!(capitalize_first_letter(""), "");
    }
}
