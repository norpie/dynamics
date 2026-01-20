//! Core matching functions for Dynamics 365 entity comparison
//! Phase 1: Excludes example-based matching for simplicity

use super::models::{MatchInfo, MatchType};
use crate::api::metadata::{EntityMetadata, FieldMetadata, FieldType, RelationshipMetadata};
use std::collections::{HashMap, HashSet};

/// Extract entities from relationships
/// Returns list of (entity_name, usage_count) tuples
pub fn extract_entities(relationships: &[RelationshipMetadata]) -> Vec<(String, usize)> {
    let mut entity_counts: HashMap<String, usize> = HashMap::new();

    for rel in relationships {
        // Skip unknown/empty entity names
        if rel.related_entity.is_empty() || rel.related_entity == "unknown" {
            continue;
        }

        *entity_counts.entry(rel.related_entity.clone()).or_insert(0) += 1;
    }

    let mut entities: Vec<(String, usize)> = entity_counts.into_iter().collect();
    entities.sort_by(|a, b| a.0.cmp(&b.0));

    entities
}

/// Apply prefix transformation to a name
/// Returns list of transformed names (supports 1-to-N prefix mappings)
fn apply_prefix_transform(
    name: &str,
    prefix_mappings: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut results = Vec::new();
    for (source_prefix, target_prefixes) in prefix_mappings {
        if let Some(suffix) = name.strip_prefix(source_prefix) {
            // Generate transformed name for each target prefix (1-to-N support)
            for target_prefix in target_prefixes {
                results.push(format!("{}{}", target_prefix, suffix));
            }
        }
    }
    results
}

/// Compute entity matches between source and target
/// Returns map of source_entity_name -> MatchInfo
/// Priority: Manual → Exact → Prefix
pub fn compute_entity_matches(
    source_entities: &[(String, usize)],
    target_entities: &[(String, usize)],
    manual_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
) -> HashMap<String, MatchInfo> {
    let mut matches = HashMap::new();

    // Build target entity lookup
    let target_lookup: HashMap<String, ()> = target_entities
        .iter()
        .map(|(name, _count)| (name.clone(), ()))
        .collect();

    for (source_name, _count) in source_entities {
        // 1. Check manual mappings first (highest priority)
        if let Some(target_names) = manual_mappings.get(source_name) {
            // Filter targets that exist in target_entities
            let valid_targets: Vec<String> = target_names
                .iter()
                .filter(|tn| target_lookup.contains_key(*tn))
                .cloned()
                .collect();

            if !valid_targets.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_targets.clone(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for target in valid_targets {
                    match_info
                        .match_types
                        .insert(target.clone(), MatchType::Manual);
                    match_info.confidences.insert(target, 1.0);
                }
                matches.insert(source_name.clone(), match_info);
                continue;
            }
        }

        // 2. Check exact name match
        if target_lookup.contains_key(source_name) {
            matches.insert(
                source_name.clone(),
                MatchInfo::single(source_name.clone(), MatchType::Exact, 1.0),
            );
            continue;
        }

        // 3. Check prefix-transformed matches (1-to-N support)
        let transformed_names = apply_prefix_transform(source_name, prefix_mappings);
        let valid_transformed: Vec<String> = transformed_names
            .iter()
            .filter(|tn| target_lookup.contains_key(*tn))
            .cloned()
            .collect();

        if !valid_transformed.is_empty() {
            let mut match_info = MatchInfo {
                target_fields: valid_transformed.clone(),
                match_types: HashMap::new(),
                confidences: HashMap::new(),
            };
            for target in valid_transformed {
                match_info
                    .match_types
                    .insert(target.clone(), MatchType::Prefix);
                match_info.confidences.insert(target, 0.9);
            }
            matches.insert(source_name.clone(), match_info);
            continue;
        }

        // No match found - don't insert anything
    }

    matches
}

/// Compute field matches between source and target
/// Returns map of source_field_name -> MatchInfo
/// Priority: Manual → Import → Exact → Prefix
pub fn compute_field_matches(
    source_fields: &[FieldMetadata],
    target_fields: &[FieldMetadata],
    manual_mappings: &HashMap<String, Vec<String>>,
    imported_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    negative_matches: &HashSet<String>,
) -> HashMap<String, MatchInfo> {
    let mut matches = HashMap::new();

    // Build target field lookup
    let target_lookup: HashMap<String, &FieldMetadata> = target_fields
        .iter()
        .map(|f| (f.logical_name.clone(), f))
        .collect();

    // Build case-insensitive target lookup for imported mappings (C# uses PascalCase)
    let target_lookup_lowercase: HashMap<String, &FieldMetadata> = target_fields
        .iter()
        .map(|f| (f.logical_name.to_lowercase(), f))
        .collect();

    // Track already matched targets to prevent duplicate matches
    let mut already_matched = HashSet::new();

    for source_field in source_fields {
        let source_name = &source_field.logical_name;

        // 1. Check manual mappings first (highest priority, 1-to-N support)
        if let Some(target_names) = manual_mappings.get(source_name) {
            let valid_targets: Vec<String> = target_names
                .iter()
                .filter(|tn| target_lookup.contains_key(*tn))
                .cloned()
                .collect();

            if !valid_targets.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_targets.clone(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for target in &valid_targets {
                    match_info
                        .match_types
                        .insert(target.clone(), MatchType::Manual);
                    match_info.confidences.insert(target.clone(), 1.0);
                    already_matched.insert(target.clone());
                }
                matches.insert(source_name.clone(), match_info);
                continue;
            }
        }

        // 2. Check imported mappings (second highest priority, 1-to-N support)
        // Use case-insensitive lookup since C# code uses PascalCase but API returns lowercase
        if let Some(target_names_cs) = imported_mappings.get(source_name) {
            let mut valid_targets = Vec::new();
            for target_name_cs in target_names_cs {
                if let Some(target_field) =
                    target_lookup_lowercase.get(&target_name_cs.to_lowercase())
                {
                    valid_targets.push(target_field.logical_name.clone());
                }
            }

            if !valid_targets.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_targets.clone(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for target in &valid_targets {
                    match_info
                        .match_types
                        .insert(target.clone(), MatchType::Import);
                    match_info.confidences.insert(target.clone(), 1.0);
                    already_matched.insert(target.clone());
                }
                matches.insert(source_name.clone(), match_info);
                continue;
            }
        }

        // 3. Check exact name match
        if let Some(target_field) = target_lookup.get(source_name) {
            let types_match = source_field.field_type == target_field.field_type;
            matches.insert(
                source_name.clone(),
                MatchInfo::single(
                    source_name.clone(),
                    if types_match {
                        MatchType::Exact
                    } else {
                        MatchType::TypeMismatch(Box::new(MatchType::Exact))
                    },
                    if types_match { 1.0 } else { 0.7 },
                ),
            );
            already_matched.insert(source_name.clone());
            continue;
        }

        // 4. Check prefix-transformed matches (1-to-N support)
        // Skip if this field is in negative_matches (blocks prefix matching)
        if !negative_matches.contains(source_name) {
            let transformed_names = apply_prefix_transform(source_name, prefix_mappings);
            let mut valid_transformed = Vec::new();
            for transformed in transformed_names {
                if let Some(target_field) = target_lookup.get(&transformed) {
                    let types_match = source_field.field_type == target_field.field_type;
                    valid_transformed.push((
                        transformed.clone(),
                        if types_match {
                            MatchType::Prefix
                        } else {
                            MatchType::TypeMismatch(Box::new(MatchType::Prefix))
                        },
                        if types_match { 0.9 } else { 0.6 },
                    ));
                }
            }

            if !valid_transformed.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_transformed
                        .iter()
                        .map(|(name, _, _)| name.clone())
                        .collect(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for (target, match_type, confidence) in valid_transformed {
                    match_info.match_types.insert(target.clone(), match_type);
                    match_info.confidences.insert(target.clone(), confidence);
                    already_matched.insert(target);
                }
                matches.insert(source_name.clone(), match_info);
                continue;
            }
        }

        // No match found - don't insert anything
    }

    matches
}

/// Check if two entity names match, considering entity mappings
fn entities_match(
    source_entity: &str,
    target_entity: &str,
    entity_matches: &HashMap<String, MatchInfo>,
) -> bool {
    // Check if source entity has a match that points to target
    if let Some(match_info) = entity_matches.get(source_entity) {
        return match_info.has_target(target_entity);
    }
    // Fallback to exact match
    source_entity == target_entity
}

/// Compute relationship matches between source and target
/// Returns map of source_relationship_name -> MatchInfo
/// Entity-aware: uses entity_matches to resolve entity type mappings
/// Priority: Manual → Exact → Prefix
pub fn compute_relationship_matches(
    source_relationships: &[RelationshipMetadata],
    target_relationships: &[RelationshipMetadata],
    manual_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    entity_matches: &HashMap<String, MatchInfo>,
) -> HashMap<String, MatchInfo> {
    let mut matches = HashMap::new();

    // Build target relationship lookup
    let target_lookup: HashMap<String, &RelationshipMetadata> = target_relationships
        .iter()
        .map(|r| (r.name.clone(), r))
        .collect();

    for source_rel in source_relationships {
        let source_name = &source_rel.name;

        // 1. Check manual mappings first (1-to-N support)
        if let Some(target_names) = manual_mappings.get(source_name) {
            let valid_targets: Vec<String> = target_names
                .iter()
                .filter(|tn| target_lookup.contains_key(*tn))
                .cloned()
                .collect();

            if !valid_targets.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_targets.clone(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for target in valid_targets {
                    match_info
                        .match_types
                        .insert(target.clone(), MatchType::Manual);
                    match_info.confidences.insert(target, 1.0);
                }
                matches.insert(source_name.clone(), match_info);
                continue;
            }
        }

        // 2. Check exact name match
        if let Some(target_rel) = target_lookup.get(source_name) {
            // Compare relationship type and related entity (entity-aware)
            let types_match = source_rel.relationship_type == target_rel.relationship_type
                && entities_match(
                    &source_rel.related_entity,
                    &target_rel.related_entity,
                    entity_matches,
                );
            matches.insert(
                source_name.clone(),
                MatchInfo::single(
                    source_name.clone(),
                    if types_match {
                        MatchType::Exact
                    } else {
                        MatchType::TypeMismatch(Box::new(MatchType::Exact))
                    },
                    if types_match { 1.0 } else { 0.7 },
                ),
            );
            continue;
        }

        // 3. Check prefix-transformed matches (1-to-N support)
        let transformed_names = apply_prefix_transform(source_name, prefix_mappings);
        let mut valid_transformed = Vec::new();
        for transformed in transformed_names {
            if let Some(target_rel) = target_lookup.get(&transformed) {
                // Compare relationship type and related entity (entity-aware)
                let types_match = source_rel.relationship_type == target_rel.relationship_type
                    && entities_match(
                        &source_rel.related_entity,
                        &target_rel.related_entity,
                        entity_matches,
                    );
                valid_transformed.push((
                    transformed.clone(),
                    if types_match {
                        MatchType::Prefix
                    } else {
                        MatchType::TypeMismatch(Box::new(MatchType::Prefix))
                    },
                    if types_match { 0.9 } else { 0.6 },
                ));
            }
        }

        if !valid_transformed.is_empty() {
            let mut match_info = MatchInfo {
                target_fields: valid_transformed
                    .iter()
                    .map(|(name, _, _)| name.clone())
                    .collect(),
                match_types: HashMap::new(),
                confidences: HashMap::new(),
            };
            for (target, match_type, confidence) in valid_transformed {
                match_info.match_types.insert(target.clone(), match_type);
                match_info.confidences.insert(target, confidence);
            }
            matches.insert(source_name.clone(), match_info);
            continue;
        }
    }

    matches
}
