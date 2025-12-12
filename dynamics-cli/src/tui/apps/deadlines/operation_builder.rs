

//! Convert TransformedDeadline records to Dynamics 365 API Operations
//!
//! This module handles the conversion of validated and transformed deadline records
//! into executable API operations, including:
//! - Main entity creation with resolved lookups
//! - Junction entity creation for N:N relationships (using Content-ID references)
//! - DateTime timezone conversion (Brussels → UTC)
//! - Proper @odata.bind formatting for lookups

use crate::api::operations::Operation;
use crate::api::pluralization::pluralize_entity_name;
use super::models::TransformedDeadline;
use super::field_mappings::get_constant_fields;
use serde_json::{json, Value};
use std::collections::HashMap;

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

        // 0. Constant fields (entity-specific defaults)
        for (field, value) in get_constant_fields(entity_type) {
            payload[field] = value;
        }

        // 1. Direct fields (name, info, etc.)
        for (field, value) in &self.direct_fields {
            payload[field] = json!(value);
        }

        // 1a. Picklist fields (integer values)
        for (field, value) in &self.picklist_fields {
            payload[field] = json!(value);
        }

        // 1b. Generate cgk_name field (formatted as "Deadline - Date Time")
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
            let date_field = if entity_type == "cgk_deadline" { "cgk_date" } else { "nrq_deadlinedate" };

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
                log::warn!("Unknown CGK relationship '{}', using fallback pattern", relationship_name);
                format!("cgk_{}", relationship_name)
            }
        }
    } else if entity_type == "nrq_deadline" {
        // NRQ: Complex pattern with capitalization - nrq_Deadline_nrq_{Entity}_nrq_{FinalEntity}
        // NOTE: nrq_support is NOT here - it uses a custom junction entity (nrq_deadlinesupport)
        // and is handled separately via custom_junction_records
        match relationship_name {
            "nrq_deadline_nrq_category" => "nrq_Deadline_nrq_Category_nrq_Category".to_string(),
            "nrq_deadline_nrq_subcategory" => "nrq_Deadline_nrq_Subcategory_nrq_Subcategory".to_string(),
            "nrq_deadline_nrq_flemishshare" => "nrq_Deadline_nrq_Flemishshare_nrq_Flemish".to_string(),
            _ => {
                log::warn!("Unknown NRQ relationship '{}', using fallback pattern", relationship_name);
                // Extract entity name and capitalize
                let entity = relationship_name.strip_prefix("nrq_deadline_nrq_")
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
    time: Option<chrono::NaiveTime>
) -> Result<String, String> {
    use chrono::{TimeZone, Utc, LocalResult};
    use chrono_tz::Europe::Brussels;

    // Use 12:00 as default if no time provided
    let local_time = time.unwrap_or_else(||
        chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap()
    );

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
            Err(format!("Invalid Brussels time (DST gap): {}", brussels_naive))
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
            "nrq_Deadline_nrq_Flemishshare_nrq_Flemish"
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
