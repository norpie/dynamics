//! Adapter/compatibility layer for migrating from old matching.rs to new service
//! This provides the old interface while using the new service underneath
//!
//! Architecture:
//! - Service provides core matching logic (Manual, Import, Exact, Prefix)
//! - Adapter augments with UI-specific features (example value matching)

use crate::api::metadata::FieldMetadata;
use crate::api::EntityMetadata;
use crate::services::matching::{self, MatchingContext, MatchingMappings};
use std::collections::{HashMap, HashSet};

// Re-export types from service for compatibility
pub use crate::services::matching::{MatchInfo, MatchType};

/// Find a target field that matches the source field's example value
/// Returns target field name if exact value match found
/// This is a UI-assistance feature, not core business logic
fn find_example_value_match(
    source_field: &FieldMetadata,
    target_fields: &[FieldMetadata],
    examples: &super::ExamplesState,
    source_entity: &str,
    target_entity: &str,
    already_matched: &HashSet<String>,
) -> Option<String> {
    // Only if examples enabled and has active pair
    if !examples.enabled || examples.get_active_pair().is_none() {
        return None;
    }

    // Get source value (skip if null/empty/boolean/0/1 - too much overlap)
    let source_value = examples.get_field_value(&source_field.logical_name, true, source_entity)?;
    if source_value == "null"
        || source_value.trim().is_empty()
        || source_value == "\"\""
        || source_value == "true"
        || source_value == "false"
        || source_value == "0"
        || source_value == "1" {
        return None;
    }

    // Find target with matching value
    for target_field in target_fields {
        if already_matched.contains(&target_field.logical_name) {
            continue;
        }

        if let Some(target_value) = examples.get_field_value(&target_field.logical_name, false, target_entity) {
            if target_value == source_value {
                return Some(target_field.logical_name.clone());
            }
        }
    }

    None
}

/// Recompute all matches (compatibility wrapper)
/// Combines service matches with app-level example matching
#[allow(clippy::too_many_arguments)]
pub fn recompute_all_matches(
    source_metadata: &EntityMetadata,
    target_metadata: &EntityMetadata,
    field_mappings: &HashMap<String, Vec<String>>,
    imported_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    examples: &super::ExamplesState,
    source_entity: &str,
    target_entity: &str,
    negative_matches: &HashSet<String>,
) -> (
    HashMap<String, MatchInfo>,  // field_matches
    HashMap<String, MatchInfo>,  // relationship_matches
    HashMap<String, MatchInfo>,  // entity_matches
    Vec<(String, usize)>,        // source_entities
    Vec<(String, usize)>,        // target_entities
) {
    // Create context for service
    let context = MatchingContext {
        source_metadata: source_metadata.clone(),
        target_metadata: target_metadata.clone(),
        source_entity: source_entity.to_string(),
        target_entity: target_entity.to_string(),
    };

    // Create mappings for service
    let mappings = MatchingMappings {
        field_mappings: field_mappings.clone(),
        prefix_mappings: prefix_mappings.clone(),
        imported_mappings: imported_mappings.clone(),
        negative_matches: negative_matches.clone(),
    };

    // Get base matches from service (Manual, Import, Exact, Prefix)
    let results = matching::compute_all_matches(&context, &mappings);
    let mut field_matches = results.field_matches;

    // Augment with example-based matches (app-level UI feature)
    // Only check unmatched fields to avoid overriding service matches
    if examples.enabled && examples.get_active_pair().is_some() {
        // Build set of already matched target fields
        let mut already_matched = HashSet::new();
        for match_info in field_matches.values() {
            for target in &match_info.target_fields {
                already_matched.insert(target.clone());
            }
        }

        // Try example matching for unmatched source fields
        for source_field in &source_metadata.fields {
            // Skip if already matched by service
            if field_matches.contains_key(&source_field.logical_name) {
                continue;
            }

            // Try to find match via example value
            if let Some(target_name) = find_example_value_match(
                source_field,
                &target_metadata.fields,
                examples,
                source_entity,
                target_entity,
                &already_matched,
            ) {
                field_matches.insert(
                    source_field.logical_name.clone(),
                    MatchInfo::single(target_name.clone(), MatchType::ExampleValue, 1.0),
                );
                already_matched.insert(target_name);
            }
        }
    }

    // Return in old format
    (
        field_matches,
        results.relationship_matches,
        results.entity_matches,
        results.source_entities,
        results.target_entities,
    )
}
