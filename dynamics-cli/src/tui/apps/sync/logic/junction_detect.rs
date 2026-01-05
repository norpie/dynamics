//! Junction entity detection logic
//!
//! This module provides functions to:
//! - Find entities that act as junction/bridge tables (N:M relationships)
//! - Suggest junction candidates based on lookup field patterns
//! - Filter already-selected entities from suggestions

use std::collections::HashSet;

use crate::api::metadata::{FieldMetadata, FieldType};
use crate::tui::apps::sync::state::JunctionCandidate;

/// Information about a potential junction entity
#[derive(Debug, Clone)]
pub struct JunctionInfo {
    /// Entity logical name
    pub logical_name: String,
    /// Entity display name
    pub display_name: Option<String>,
    /// Entities this junction connects (from lookup fields)
    pub connected_entities: Vec<String>,
    /// Number of connections to selected entities
    pub connection_count: usize,
}

/// Find junction entity candidates from a list of all entities
///
/// A junction entity is one that:
/// 1. Is NOT already in the selected set
/// 2. Has 2+ lookup fields pointing to entities IN the selected set
///
/// # Arguments
/// * `selected_entities` - Set of entity names the user has selected
/// * `all_entities` - All available entities with their fields
///
/// # Returns
/// List of junction candidates, sorted by number of connections (descending)
pub fn find_junction_candidates(
    selected_entities: &HashSet<String>,
    all_entities: &[(String, Option<String>, Vec<FieldMetadata>)],
) -> Vec<JunctionCandidate> {
    let mut candidates = Vec::new();

    for (entity_name, display_name, fields) in all_entities {
        // Skip if already selected
        if selected_entities.contains(entity_name) {
            continue;
        }

        // Find lookup fields pointing to selected entities
        let connections: Vec<String> = fields
            .iter()
            .filter(|f| matches!(f.field_type, FieldType::Lookup))
            .filter_map(|f| f.related_entity.as_ref())
            .filter(|target| selected_entities.contains(*target))
            .cloned()
            .collect();

        // Unique connections (might have multiple lookups to same entity)
        let unique_connections: HashSet<_> = connections.iter().collect();

        // Junction needs 2+ unique connections to selected entities
        if unique_connections.len() >= 2 {
            candidates.push(JunctionCandidate {
                logical_name: entity_name.clone(),
                display_name: display_name.clone(),
                connects: unique_connections.into_iter().cloned().collect(),
            });
        }
    }

    // Sort by number of connections (most connections first), then by name
    candidates.sort_by(|a, b| {
        b.connects.len().cmp(&a.connects.len())
            .then_with(|| a.logical_name.cmp(&b.logical_name))
    });

    candidates
}

/// Check if an entity is likely a junction table based on its structure
///
/// Heuristics used:
/// - Has primarily lookup fields (more than 50% lookups)
/// - Has few non-system fields total
/// - Name contains common junction patterns
pub fn is_likely_junction(
    entity_name: &str,
    fields: &[FieldMetadata],
) -> bool {
    // Name-based heuristics
    let name_lower = entity_name.to_lowercase();
    let junction_patterns = [
        "_connection",
        "_association",
        "_link",
        "_member",
        "_x_",
    ];

    if junction_patterns.iter().any(|p| name_lower.contains(p)) {
        return true;
    }

    // Field-based heuristics
    let non_system_fields: Vec<_> = fields
        .iter()
        .filter(|f| !crate::tui::apps::sync::types::is_system_field(&f.logical_name))
        .collect();

    let lookup_count = non_system_fields
        .iter()
        .filter(|f| matches!(f.field_type, FieldType::Lookup))
        .count();

    // If most non-system fields are lookups and there are few fields total,
    // it's likely a junction table
    let total = non_system_fields.len();
    if total > 0 && total <= 10 && lookup_count >= 2 {
        let lookup_ratio = lookup_count as f64 / total as f64;
        if lookup_ratio >= 0.5 {
            return true;
        }
    }

    false
}

/// Extract the entities that a junction connects
pub fn get_junction_connections(
    fields: &[FieldMetadata],
    filter_to: Option<&HashSet<String>>,
) -> Vec<String> {
    let targets: Vec<String> = fields
        .iter()
        .filter(|f| matches!(f.field_type, FieldType::Lookup))
        .filter_map(|f| f.related_entity.clone())
        .collect();

    let unique: HashSet<_> = targets.into_iter().collect();

    let mut result: Vec<_> = if let Some(filter) = filter_to {
        unique.into_iter().filter(|t| filter.contains(t)).collect()
    } else {
        unique.into_iter().collect()
    };

    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lookup(name: &str, target: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            schema_name: None,
            display_name: Some(name.to_string()),
            field_type: FieldType::Lookup,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: Some(target.to_string()),
            option_values: vec![],
        }
    }

    fn make_string_field(name: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            schema_name: None,
            display_name: Some(name.to_string()),
            field_type: FieldType::String,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: None,
            option_values: vec![],
        }
    }

    #[test]
    fn test_find_junction_basic() {
        let selected: HashSet<_> = ["account", "contact"].iter().map(|s| s.to_string()).collect();

        let all_entities = vec![
            // Selected entities (should be ignored)
            ("account".to_string(), None, vec![make_string_field("name")]),
            ("contact".to_string(), None, vec![make_string_field("name")]),
            // Junction candidate
            ("account_contact".to_string(), Some("Account Contact".to_string()), vec![
                make_lookup("accountid", "account"),
                make_lookup("contactid", "contact"),
            ]),
            // Not a junction - only 1 connection
            ("opportunity".to_string(), None, vec![
                make_lookup("accountid", "account"),
                make_string_field("name"),
            ]),
        ];

        let candidates = find_junction_candidates(&selected, &all_entities);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].logical_name, "account_contact");
        assert_eq!(candidates[0].connects.len(), 2);
    }

    #[test]
    fn test_find_junction_ignores_selected() {
        let selected: HashSet<_> = ["account", "contact", "account_contact"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let all_entities = vec![
            ("account".to_string(), None, vec![make_string_field("name")]),
            ("contact".to_string(), None, vec![make_string_field("name")]),
            // Already selected - should NOT appear as candidate
            ("account_contact".to_string(), None, vec![
                make_lookup("accountid", "account"),
                make_lookup("contactid", "contact"),
            ]),
        ];

        let candidates = find_junction_candidates(&selected, &all_entities);

        assert!(candidates.is_empty());
    }

    #[test]
    fn test_find_junction_external_lookups_ignored() {
        let selected: HashSet<_> = ["account", "contact"].iter().map(|s| s.to_string()).collect();

        let all_entities = vec![
            ("account".to_string(), None, vec![make_string_field("name")]),
            ("contact".to_string(), None, vec![make_string_field("name")]),
            // Has lookups but only 1 to selected entities
            ("some_entity".to_string(), None, vec![
                make_lookup("accountid", "account"),
                make_lookup("systemuserid", "systemuser"), // External
            ]),
        ];

        let candidates = find_junction_candidates(&selected, &all_entities);

        // Should not be a candidate - only 1 connection to selected
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_is_likely_junction_by_name() {
        assert!(is_likely_junction("account_contact_connection", &[]));
        assert!(is_likely_junction("user_x_role", &[]));
        assert!(is_likely_junction("team_member", &[]));
        assert!(!is_likely_junction("account", &[]));
    }

    #[test]
    fn test_is_likely_junction_by_fields() {
        let junction_fields = vec![
            make_lookup("accountid", "account"),
            make_lookup("contactid", "contact"),
            make_string_field("description"),
        ];

        assert!(is_likely_junction("my_entity", &junction_fields));

        let normal_fields = vec![
            make_string_field("name"),
            make_string_field("description"),
            make_string_field("email"),
            make_lookup("ownerid", "systemuser"),
        ];

        assert!(!is_likely_junction("my_entity", &normal_fields));
    }

    #[test]
    fn test_get_junction_connections() {
        let fields = vec![
            make_lookup("accountid", "account"),
            make_lookup("contactid", "contact"),
            make_lookup("secondaccountid", "account"), // Duplicate target
            make_string_field("name"),
        ];

        let connections = get_junction_connections(&fields, None);

        assert_eq!(connections.len(), 2); // Unique targets
        assert!(connections.contains(&"account".to_string()));
        assert!(connections.contains(&"contact".to_string()));
    }

    #[test]
    fn test_get_junction_connections_filtered() {
        let fields = vec![
            make_lookup("accountid", "account"),
            make_lookup("contactid", "contact"),
            make_lookup("systemuserid", "systemuser"),
        ];

        let filter: HashSet<_> = ["account", "contact"].iter().map(|s| s.to_string()).collect();
        let connections = get_junction_connections(&fields, Some(&filter));

        assert_eq!(connections.len(), 2);
        assert!(!connections.contains(&"systemuser".to_string()));
    }
}
