//! Fetch existing deadlines from Dynamics 365 for edit/update support.
//!
//! This module handles:
//! - Chunked fetching of all existing deadlines (500 per request)
//! - Expanding N:N associations in the same query
//! - Building a lookup map for matching by (name, date)

use std::collections::{HashMap, HashSet};
use chrono::NaiveDate;
use serde_json::Value;

use crate::api::QueryBuilder;
use crate::api::query::orderby::OrderBy;

use super::models::{
    DeadlineLookupKey, DeadlineLookupMap, ExistingAssociations, ExistingDeadline,
    ExistingJunctionRecord,
};

/// Chunk size for fetching deadlines (Dynamics API limit considerations)
const FETCH_CHUNK_SIZE: u32 = 500;

/// Navigation property names for CGK deadline N:N relationships
mod cgk_nav_props {
    pub const SUPPORT: &str = "cgk_cgk_deadline_cgk_support";
    pub const CATEGORY: &str = "cgk_cgk_deadline_cgk_category";
    pub const LENGTH: &str = "cgk_cgk_deadline_cgk_length";
    pub const FLEMISHSHARE: &str = "cgk_cgk_flemishshare_cgk_deadline"; // Reversed!
}

/// Navigation property names for NRQ deadline N:N relationships
mod nrq_nav_props {
    pub const CATEGORY: &str = "nrq_Deadline_nrq_Category_nrq_Category";
    pub const SUBCATEGORY: &str = "nrq_Deadline_nrq_Subcategory_nrq_Subcategory";
    pub const FLEMISHSHARE: &str = "nrq_Deadline_nrq_FlemishShare_nrq_Flemish";
    // Note: NRQ support uses custom junction entity, not N:N
}

/// Fetch all existing deadlines with their associations.
///
/// Returns a lookup map keyed by (name_lowercase, date) for matching.
pub async fn fetch_existing_deadlines(
    entity_type: &str,
) -> Result<DeadlineLookupMap, String> {
    let manager = crate::client_manager();
    let client = manager
        .get_current_client()
        .await
        .map_err(|e| format!("Failed to get client: {}", e))?;

    let is_cgk = entity_type == "cgk_deadline";
    let entity_set = if is_cgk { "cgk_deadlines" } else { "nrq_deadlines" };

    // Build the $expand expressions for N:N relationships
    let expand_expressions = build_expand_expressions(entity_type);

    // Build select fields for main entity
    let select_fields: Vec<&str> = if is_cgk {
        vec!["cgk_deadlineid", "cgk_deadlinename", "cgk_date"]
    } else {
        vec!["nrq_deadlineid", "nrq_deadlinename", "nrq_deadlinedate"]
    };

    // Order by field
    let orderby_field = if is_cgk { "cgk_date" } else { "nrq_deadlinedate" };

    let mut all_deadlines: Vec<ExistingDeadline> = Vec::new();
    let mut skip: u32 = 0;

    loop {
        // Build query with pagination
        let query = QueryBuilder::new(entity_set)
            .select(&select_fields)
            .expand(&expand_expressions.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .orderby(OrderBy::desc(orderby_field))
            .top(FETCH_CHUNK_SIZE)
            .skip(skip)
            .build();

        log::debug!("Fetching existing deadlines (skip={})", skip);

        let result = client
            .execute_query(&query)
            .await
            .map_err(|e| format!("Failed to fetch deadlines: {}", e))?;

        let records = result.records().cloned().unwrap_or_default();
        let record_count = records.len();

        log::debug!("Fetched {} deadline records (skip={})", record_count, skip);

        // Parse each record into ExistingDeadline
        for record in records {
            if let Some(deadline) = parse_deadline_record(&record, entity_type) {
                all_deadlines.push(deadline);
            }
        }

        // Check if we got fewer records than requested (last page)
        if (record_count as u32) < FETCH_CHUNK_SIZE {
            break;
        }

        skip += FETCH_CHUNK_SIZE;
    }

    log::info!(
        "Fetched {} total existing deadlines for {}",
        all_deadlines.len(),
        entity_type
    );

    // For NRQ, we also need to fetch custom junction records (nrq_deadlinesupport)
    if !is_cgk {
        fetch_nrq_support_junctions(&client, &mut all_deadlines).await?;
    }

    // Build lookup map
    let lookup_map = build_lookup_map(all_deadlines);

    Ok(lookup_map)
}

/// Build the $expand expressions for fetching N:N associations
fn build_expand_expressions(entity_type: &str) -> Vec<String> {
    if entity_type == "cgk_deadline" {
        vec![
            format!("{}($select=cgk_supportid,cgk_name)", cgk_nav_props::SUPPORT),
            format!("{}($select=cgk_categoryid,cgk_name)", cgk_nav_props::CATEGORY),
            format!("{}($select=cgk_lengthid,cgk_name)", cgk_nav_props::LENGTH),
            format!("{}($select=cgk_flemishshareid,cgk_name)", cgk_nav_props::FLEMISHSHARE),
        ]
    } else {
        // NRQ: no support in expand (uses custom junction)
        vec![
            format!("{}($select=nrq_categoryid,nrq_name)", nrq_nav_props::CATEGORY),
            format!("{}($select=nrq_subcategoryid,nrq_name)", nrq_nav_props::SUBCATEGORY),
            format!("{}($select=nrq_flemishshareid,nrq_name)", nrq_nav_props::FLEMISHSHARE),
        ]
    }
}

/// Parse a deadline record from the API response
fn parse_deadline_record(record: &Value, entity_type: &str) -> Option<ExistingDeadline> {
    let is_cgk = entity_type == "cgk_deadline";

    // Extract ID
    let id_field = if is_cgk { "cgk_deadlineid" } else { "nrq_deadlineid" };
    let id = record.get(id_field)?.as_str()?.to_string();

    // Extract name
    let name_field = if is_cgk { "cgk_deadlinename" } else { "nrq_deadlinename" };
    let name = record.get(name_field)?.as_str()?.to_string();

    // Extract date (parse from ISO string, extract date portion only)
    let date_field = if is_cgk { "cgk_date" } else { "nrq_deadlinedate" };
    let date_str = record.get(date_field)?.as_str()?;
    let date = parse_date_from_iso(date_str)?;

    // Extract associations
    let associations = parse_associations(record, entity_type);

    // Store all fields for later diffing
    let fields = record
        .as_object()
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();

    Some(ExistingDeadline {
        id,
        name,
        date,
        fields,
        associations,
    })
}

/// Parse N:N associations from expanded navigation properties
fn parse_associations(record: &Value, entity_type: &str) -> ExistingAssociations {
    let mut associations = ExistingAssociations::default();
    let is_cgk = entity_type == "cgk_deadline";

    if is_cgk {
        // CGK associations
        associations.support_ids = extract_ids_from_nav_prop(
            record,
            cgk_nav_props::SUPPORT,
            "cgk_supportid",
        );
        associations.category_ids = extract_ids_from_nav_prop(
            record,
            cgk_nav_props::CATEGORY,
            "cgk_categoryid",
        );
        associations.length_ids = extract_ids_from_nav_prop(
            record,
            cgk_nav_props::LENGTH,
            "cgk_lengthid",
        );
        associations.flemishshare_ids = extract_ids_from_nav_prop(
            record,
            cgk_nav_props::FLEMISHSHARE,
            "cgk_flemishshareid",
        );
    } else {
        // NRQ associations (support handled separately via junction)
        associations.category_ids = extract_ids_from_nav_prop(
            record,
            nrq_nav_props::CATEGORY,
            "nrq_categoryid",
        );
        associations.subcategory_ids = extract_ids_from_nav_prop(
            record,
            nrq_nav_props::SUBCATEGORY,
            "nrq_subcategoryid",
        );
        associations.flemishshare_ids = extract_ids_from_nav_prop(
            record,
            nrq_nav_props::FLEMISHSHARE,
            "nrq_flemishshareid",
        );
    }

    associations
}

/// Extract IDs from an expanded navigation property array
fn extract_ids_from_nav_prop(
    record: &Value,
    nav_prop: &str,
    id_field: &str,
) -> HashSet<String> {
    let mut ids = HashSet::new();

    if let Some(array) = record.get(nav_prop).and_then(|v| v.as_array()) {
        for item in array {
            if let Some(id) = item.get(id_field).and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }

    ids
}

/// Fetch NRQ support junction records (nrq_deadlinesupport)
async fn fetch_nrq_support_junctions(
    client: &crate::api::DynamicsClient,
    deadlines: &mut [ExistingDeadline],
) -> Result<(), String> {
    if deadlines.is_empty() {
        return Ok(());
    }

    // Build a map from deadline ID to index for fast lookup
    let deadline_id_to_idx: HashMap<String, usize> = deadlines
        .iter()
        .enumerate()
        .map(|(idx, d)| (d.id.clone(), idx))
        .collect();

    // Fetch all junction records in chunks
    let mut skip: u32 = 0;

    loop {
        let query = QueryBuilder::new("nrq_deadlinesupports")
            .select(&["nrq_deadlinesupportid", "_nrq_deadlineid_value", "_nrq_supportid_value", "nrq_name"])
            .top(FETCH_CHUNK_SIZE)
            .skip(skip)
            .build();

        log::debug!("Fetching NRQ support junctions (skip={})", skip);

        let result = client
            .execute_query(&query)
            .await
            .map_err(|e| format!("Failed to fetch support junctions: {}", e))?;

        let records = result.records().cloned().unwrap_or_default();
        let record_count = records.len();

        log::debug!("Fetched {} junction records (skip={})", record_count, skip);

        // Associate junction records with their deadlines
        for record in records {
            if let (Some(junction_id), Some(deadline_id), Some(support_id)) = (
                record.get("nrq_deadlinesupportid").and_then(|v| v.as_str()),
                record.get("_nrq_deadlineid_value").and_then(|v| v.as_str()),
                record.get("_nrq_supportid_value").and_then(|v| v.as_str()),
            ) {
                // Find the deadline this junction belongs to
                if let Some(&idx) = deadline_id_to_idx.get(deadline_id) {
                    let name = record
                        .get("nrq_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    deadlines[idx].associations.support_ids.insert(support_id.to_string());
                    deadlines[idx].associations.custom_junction_records.push(
                        ExistingJunctionRecord {
                            junction_id: junction_id.to_string(),
                            related_id: support_id.to_string(),
                            related_name: name,
                        },
                    );
                }
            }
        }

        if (record_count as u32) < FETCH_CHUNK_SIZE {
            break;
        }

        skip += FETCH_CHUNK_SIZE;
    }

    Ok(())
}

/// Parse a date from ISO 8601 format (e.g., "2026-12-19T11:00:00Z")
fn parse_date_from_iso(date_str: &str) -> Option<NaiveDate> {
    // Try parsing as full ISO datetime
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.date_naive());
    }

    // Try parsing with Z suffix but no timezone offset
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%SZ") {
        return Some(dt.date());
    }

    // Try parsing with milliseconds
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%.fZ") {
        return Some(dt.date());
    }

    // Try parsing as date only
    if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Some(d);
    }

    log::warn!("Failed to parse date from: {}", date_str);
    None
}

/// Build lookup map from deadlines, keyed by (name_lowercase, date)
fn build_lookup_map(deadlines: Vec<ExistingDeadline>) -> DeadlineLookupMap {
    let mut map = DeadlineLookupMap::new();

    for deadline in deadlines {
        let key: DeadlineLookupKey = (deadline.name.trim().to_lowercase(), deadline.date);

        if map.contains_key(&key) {
            log::warn!(
                "Duplicate deadline key found: ({}, {}). Keeping first occurrence.",
                key.0,
                key.1
            );
        } else {
            map.insert(key, deadline);
        }
    }

    log::info!("Built lookup map with {} unique entries", map.len());
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_from_iso() {
        assert_eq!(
            parse_date_from_iso("2026-12-19T11:00:00Z"),
            Some(NaiveDate::from_ymd_opt(2026, 12, 19).unwrap())
        );
        assert_eq!(
            parse_date_from_iso("2026-12-19"),
            Some(NaiveDate::from_ymd_opt(2026, 12, 19).unwrap())
        );
    }

    #[test]
    fn test_build_lookup_map() {
        let deadlines = vec![
            ExistingDeadline {
                id: "id1".to_string(),
                name: "Test Deadline".to_string(),
                date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                fields: HashMap::new(),
                associations: ExistingAssociations::default(),
            },
            ExistingDeadline {
                id: "id2".to_string(),
                name: "Another Deadline".to_string(),
                date: NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
                fields: HashMap::new(),
                associations: ExistingAssociations::default(),
            },
        ];

        let map = build_lookup_map(deadlines);
        assert_eq!(map.len(), 2);

        // Lookup is case-insensitive
        let key = ("test deadline".to_string(), NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert!(map.contains_key(&key));
    }
}
