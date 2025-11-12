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

/// Recompute all matches for N:M entity comparison
/// Returns qualified field names (e.g., "contact.fullname" -> MatchInfo with "lead.firstname")
#[allow(clippy::too_many_arguments)]
pub fn recompute_all_matches_multi(
    source_metadata_map: &HashMap<String, EntityMetadata>,
    target_metadata_map: &HashMap<String, EntityMetadata>,
    source_entities: &[String],
    target_entities: &[String],
    field_mappings: &HashMap<String, Vec<String>>,
    imported_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    examples: &super::ExamplesState,
    negative_matches: &HashSet<String>,
) -> (
    HashMap<String, MatchInfo>,  // field_matches (qualified keys)
    HashMap<String, MatchInfo>,  // relationship_matches (qualified keys)
    HashMap<String, MatchInfo>,  // entity_matches (qualified keys)
    Vec<(String, usize)>,        // source_related_entities
    Vec<(String, usize)>,        // target_related_entities
) {
    let mut all_field_matches = HashMap::new();
    let mut all_relationship_matches = HashMap::new();
    let mut all_entity_matches = HashMap::new();
    let mut all_source_entities = Vec::new();
    let mut all_target_entities = Vec::new();

    // Compute matches for each source/target entity pair
    for source_entity in source_entities {
        for target_entity in target_entities {
            // Get metadata for this pair
            let source_meta = match source_metadata_map.get(source_entity) {
                Some(meta) => meta,
                None => continue,
            };
            let target_meta = match target_metadata_map.get(target_entity) {
                Some(meta) => meta,
                None => continue,
            };

            // Filter mappings to this entity pair only
            let filtered_field_mappings = filter_mappings_for_entities(field_mappings, source_entity, target_entity);
            let filtered_imported_mappings = filter_mappings_for_entities(imported_mappings, source_entity, target_entity);
            let filtered_prefix_mappings = filter_mappings_for_entities(prefix_mappings, source_entity, target_entity);
            let filtered_negative_matches = filter_negative_matches(negative_matches, source_entity, target_entity);

            // Compute matches for this pair
            let (field_matches, relationship_matches, entity_matches, source_ents, target_ents) =
                recompute_all_matches(
                    source_meta,
                    target_meta,
                    &filtered_field_mappings,
                    &filtered_imported_mappings,
                    &filtered_prefix_mappings,
                    examples,
                    source_entity,
                    target_entity,
                    &filtered_negative_matches,
                );

            // Qualify and merge field matches
            for (source_field, match_info) in field_matches {
                let qualified_source = format!("{}.{}", source_entity, source_field);
                let qualified_targets: Vec<String> = match_info.target_fields.iter()
                    .map(|t| format!("{}.{}", target_entity, t))
                    .collect();

                let qualified_match_types: HashMap<String, MatchType> = match_info.match_types.iter()
                    .map(|(t, mt)| (format!("{}.{}", target_entity, t), mt.clone()))
                    .collect();

                let qualified_confidences: HashMap<String, f64> = match_info.confidences.iter()
                    .map(|(t, c)| (format!("{}.{}", target_entity, t), *c))
                    .collect();

                all_field_matches.insert(qualified_source, MatchInfo {
                    target_fields: qualified_targets,
                    match_types: qualified_match_types,
                    confidences: qualified_confidences,
                });
            }

            // Qualify and merge relationship matches
            for (source_rel, match_info) in relationship_matches {
                let qualified_source = format!("{}.{}", source_entity, source_rel);
                let qualified_targets: Vec<String> = match_info.target_fields.iter()
                    .map(|t| format!("{}.{}", target_entity, t))
                    .collect();

                let qualified_match_types: HashMap<String, MatchType> = match_info.match_types.iter()
                    .map(|(t, mt)| (format!("{}.{}", target_entity, t), mt.clone()))
                    .collect();

                let qualified_confidences: HashMap<String, f64> = match_info.confidences.iter()
                    .map(|(t, c)| (format!("{}.{}", target_entity, t), *c))
                    .collect();

                all_relationship_matches.insert(qualified_source, MatchInfo {
                    target_fields: qualified_targets,
                    match_types: qualified_match_types,
                    confidences: qualified_confidences,
                });
            }

            // Merge entity matches (no qualification needed - entity names are already unique)
            all_entity_matches.extend(entity_matches);

            // Merge related entities
            all_source_entities.extend(source_ents);
            all_target_entities.extend(target_ents);
        }
    }

    // Deduplicate related entities by name and sum usage counts
    all_source_entities = deduplicate_entities(all_source_entities);
    all_target_entities = deduplicate_entities(all_target_entities);

    (
        all_field_matches,
        all_relationship_matches,
        all_entity_matches,
        all_source_entities,
        all_target_entities,
    )
}

/// Filter mappings to only include entries for specific source/target entity pair
/// Mappings are stored with qualified names: "entity.field" -> ["entity.field", ...]
fn filter_mappings_for_entities(
    mappings: &HashMap<String, Vec<String>>,
    source_entity: &str,
    target_entity: &str,
) -> HashMap<String, Vec<String>> {
    let source_prefix = format!("{}.", source_entity);
    let target_prefix = format!("{}.", target_entity);

    let mut filtered = HashMap::new();

    for (source_qualified, targets_qualified) in mappings {
        // Check if source matches this entity
        if let Some(source_field) = source_qualified.strip_prefix(&source_prefix) {
            // Filter targets to only this target entity and strip prefix
            let filtered_targets: Vec<String> = targets_qualified.iter()
                .filter_map(|t| t.strip_prefix(&target_prefix).map(|s| s.to_string()))
                .collect();

            if !filtered_targets.is_empty() {
                filtered.insert(source_field.to_string(), filtered_targets);
            }
        }
    }

    filtered
}

/// Filter negative matches to only include entries for specific entity pair
/// Negative matches are stored as "source_entity.source_field:target_entity.target_field"
fn filter_negative_matches(
    negative_matches: &HashSet<String>,
    source_entity: &str,
    target_entity: &str,
) -> HashSet<String> {
    let mut filtered = HashSet::new();

    for neg_match in negative_matches {
        if let Some((source_qualified, target_qualified)) = neg_match.split_once(':') {
            // Parse source
            if let Some((src_entity, src_field)) = source_qualified.split_once('.') {
                // Parse target
                if let Some((tgt_entity, tgt_field)) = target_qualified.split_once('.') {
                    // Check if this negative match applies to this entity pair
                    if src_entity == source_entity && tgt_entity == target_entity {
                        // Add unqualified version for service
                        filtered.insert(format!("{}:{}", src_field, tgt_field));
                    }
                }
            }
        }
    }

    filtered
}

/// Deduplicate entities by name and sum their usage counts
fn deduplicate_entities(entities: Vec<(String, usize)>) -> Vec<(String, usize)> {
    let mut map: HashMap<String, usize> = HashMap::new();

    for (name, count) in entities {
        *map.entry(name).or_insert(0) += count;
    }

    let mut result: Vec<_> = map.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}
