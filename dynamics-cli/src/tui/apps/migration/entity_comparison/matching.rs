//! Matching logic for fields, relationships, and containers

use super::models::{MatchInfo, MatchType};
use crate::api::metadata::{FieldMetadata, RelationshipMetadata};
use std::collections::HashMap;

/// Compute entity matches between source and target
/// Returns map of source_entity_name -> MatchInfo
/// Uses identical logic to field matching: manual → exact → prefix
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
            let valid_targets: Vec<String> = target_names.iter()
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
                    match_info.match_types.insert(target.clone(), MatchType::Manual);
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
        let valid_transformed: Vec<String> = transformed_names.iter()
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
                match_info.match_types.insert(target.clone(), MatchType::Prefix);
                match_info.confidences.insert(target, 0.9);
            }
            matches.insert(source_name.clone(), match_info);
            continue;
        }

        // No match found - don't insert anything
    }

    matches
}

/// Find a target field that matches the source field's example value
/// Returns target field name if exact value match found
fn find_example_value_match(
    source_field: &FieldMetadata,
    target_fields: &[FieldMetadata],
    examples: &super::ExamplesState,
    source_entity: &str,
    target_entity: &str,
    already_matched: &std::collections::HashSet<String>,
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

/// Compute field matches between source and target
/// Returns map of source_field_name -> MatchInfo
/// Priority: Manual > Import > Exact > Prefix > Example
pub fn compute_field_matches(
    source_fields: &[FieldMetadata],
    target_fields: &[FieldMetadata],
    manual_mappings: &HashMap<String, Vec<String>>,
    imported_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    examples: &super::ExamplesState,
    source_entity: &str,
    target_entity: &str,
    negative_matches: &std::collections::HashSet<String>,
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
    let mut already_matched = std::collections::HashSet::new();

    for source_field in source_fields {
        let source_name = &source_field.logical_name;

        // 1. Check manual mappings first (highest priority, 1-to-N support)
        if let Some(target_names) = manual_mappings.get(source_name) {
            let valid_targets: Vec<String> = target_names.iter()
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
                    match_info.match_types.insert(target.clone(), MatchType::Manual);
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
                if let Some(target_field) = target_lookup_lowercase.get(&target_name_cs.to_lowercase()) {
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
                    match_info.match_types.insert(target.clone(), MatchType::Import);
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
                    if types_match { MatchType::Exact } else { MatchType::TypeMismatch(Box::new(MatchType::Exact)) },
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
                        if types_match { MatchType::Prefix } else { MatchType::TypeMismatch(Box::new(MatchType::Prefix)) },
                        if types_match { 0.9 } else { 0.6 },
                    ));
                }
            }

            if !valid_transformed.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_transformed.iter().map(|(name, _, _)| name.clone()).collect(),
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

        // 5. Check example-based matching (only for unmatched fields)
        if let Some(target_name) = find_example_value_match(
            source_field,
            target_fields,
            examples,
            source_entity,
            target_entity,
            &already_matched,
        ) {
            matches.insert(
                source_name.clone(),
                MatchInfo::single(target_name.clone(), MatchType::ExampleValue, 1.0),
            );
            already_matched.insert(target_name);
            continue;
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
/// Now entity-aware: uses entity_matches to resolve entity type mappings
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
            let valid_targets: Vec<String> = target_names.iter()
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
                    match_info.match_types.insert(target.clone(), MatchType::Manual);
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
                && entities_match(&source_rel.related_entity, &target_rel.related_entity, entity_matches);
            matches.insert(
                source_name.clone(),
                MatchInfo::single(
                    source_name.clone(),
                    if types_match { MatchType::Exact } else { MatchType::TypeMismatch(Box::new(MatchType::Exact)) },
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
                    && entities_match(&source_rel.related_entity, &target_rel.related_entity, entity_matches);
                valid_transformed.push((
                    transformed.clone(),
                    if types_match { MatchType::Prefix } else { MatchType::TypeMismatch(Box::new(MatchType::Prefix)) },
                    if types_match { 0.9 } else { 0.6 },
                ));
            }
        }

        if !valid_transformed.is_empty() {
            let mut match_info = MatchInfo {
                target_fields: valid_transformed.iter().map(|(name, _, _)| name.clone()).collect(),
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

/// Apply prefix transformation to a name
/// Returns transformed name if any prefix mapping applies
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

/// Match container names (Forms, Views, Tabs, Sections, etc.)
/// Returns map of source_name -> target_name for matched containers
pub fn match_container_names(
    source_names: &[String],
    target_names: &[String],
) -> HashMap<String, String> {
    let mut matches = HashMap::new();

    let target_set: HashMap<String, ()> = target_names
        .iter()
        .map(|n| (n.clone(), ()))
        .collect();

    for source_name in source_names {
        // Simple exact match for now
        // TODO: Could add fuzzy matching, prefix transforms, etc.
        if target_set.contains_key(source_name) {
            matches.insert(source_name.clone(), source_name.clone());
        }
    }

    matches
}

/// Compute hierarchical field matches for Forms/Views tabs
/// Uses full path matching: fields only match if their container paths also match
/// Priority: Manual > Import > Exact > Prefix
pub fn compute_hierarchical_field_matches(
    source_metadata: &crate::api::EntityMetadata,
    target_metadata: &crate::api::EntityMetadata,
    manual_mappings: &HashMap<String, Vec<String>>,
    imported_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    tab_type: &str, // "forms" or "views"
    negative_matches: &std::collections::HashSet<String>,
) -> HashMap<String, MatchInfo> {
    let mut matches = HashMap::new();

    // Build path maps for source and target
    let source_paths = build_metadata_paths(source_metadata, tab_type);
    let target_paths = build_metadata_paths(target_metadata, tab_type);

    // Separate containers and fields
    let mut source_containers = Vec::new();
    let mut source_fields_by_container: HashMap<String, Vec<PathInfo>> = HashMap::new();

    for path_info in source_paths {
        if path_info.is_field {
            // Extract parent container path (everything before last /)
            if let Some(parent_path) = path_info.path.rfind('/').map(|i| &path_info.path[..i]) {
                source_fields_by_container.entry(parent_path.to_string()).or_default().push(path_info);
            }
        } else {
            source_containers.push(path_info);
        }
    }

    let mut target_containers = Vec::new();
    let mut target_fields_by_container: HashMap<String, Vec<PathInfo>> = HashMap::new();

    for path_info in target_paths {
        if path_info.is_field {
            // Extract parent container path
            if let Some(parent_path) = path_info.path.rfind('/').map(|i| &path_info.path[..i]) {
                target_fields_by_container.entry(parent_path.to_string()).or_default().push(path_info);
            }
        } else {
            target_containers.push(path_info);
        }
    }

    // Create target container lookup
    let target_container_lookup: HashMap<String, &PathInfo> = target_containers
        .iter()
        .map(|p| (p.path.clone(), p))
        .collect();

    // Match containers first (exact path only)
    for source_container in &source_containers {
        let source_path = &source_container.path;

        // Check manual mapping for container (1-to-N support)
        if let Some(target_paths) = manual_mappings.get(source_path) {
            let valid_targets: Vec<String> = target_paths.iter()
                .filter(|tp| target_container_lookup.contains_key(*tp))
                .cloned()
                .collect();

            if !valid_targets.is_empty() {
                let mut match_info = MatchInfo {
                    target_fields: valid_targets.clone(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for target in valid_targets {
                    match_info.match_types.insert(target.clone(), MatchType::Manual);
                    match_info.confidences.insert(target, 1.0);
                }
                matches.insert(source_path.clone(), match_info);
                continue;
            }
        }

        // Check exact path match
        if target_container_lookup.contains_key(source_path) {
            matches.insert(
                source_path.clone(),
                MatchInfo::single(source_path.clone(), MatchType::Exact, 1.0),
            );
        }
    }

    // Match fields within containers
    for (container_path, source_fields) in &source_fields_by_container {
        // Only match fields if their container matched
        if !matches.contains_key(container_path) {
            continue;
        }

        // Get corresponding target container path
        let target_container_path = matches.get(container_path).unwrap().primary_target().unwrap();

        // Get fields in target container
        let target_fields = match target_fields_by_container.get(target_container_path) {
            Some(fields) => fields,
            None => continue,
        };

        // Build lookup for target fields by name
        let target_field_lookup: HashMap<String, &PathInfo> = target_fields
            .iter()
            .filter_map(|p| {
                p.path.rfind('/').map(|i| (p.path[i+1..].to_string(), p))
            })
            .collect();

        // Build case-insensitive lookup for imported mappings (C# uses PascalCase)
        let target_field_lookup_lowercase: HashMap<String, &PathInfo> = target_fields
            .iter()
            .filter_map(|p| {
                p.path.rfind('/').map(|i| (p.path[i+1..].to_lowercase(), p))
            })
            .collect();

        // Match each source field
        for source_field in source_fields {
            let source_field_name = source_field.path.rfind('/').map(|i| &source_field.path[i+1..]).unwrap_or(&source_field.path);

            // 1. Check manual mapping (1-to-N support)
            if let Some(target_paths) = manual_mappings.get(&source_field.path) {
                let mut match_info = MatchInfo {
                    target_fields: target_paths.clone(),
                    match_types: HashMap::new(),
                    confidences: HashMap::new(),
                };
                for target in target_paths {
                    match_info.match_types.insert(target.clone(), MatchType::Manual);
                    match_info.confidences.insert(target.clone(), 1.0);
                }
                matches.insert(source_field.path.clone(), match_info);
                continue;
            }

            // 2. Check imported mapping (by field name only, not full path, 1-to-N support)
            // Use case-insensitive lookup since C# code uses PascalCase but API returns lowercase
            if let Some(target_names_cs) = imported_mappings.get(source_field_name as &str) {
                let mut valid_targets = Vec::new();
                for target_name_cs in target_names_cs {
                    if let Some(target_field) = target_field_lookup_lowercase.get(&target_name_cs.to_lowercase()) {
                        valid_targets.push(target_field.path.clone());
                    }
                }

                if !valid_targets.is_empty() {
                    let mut match_info = MatchInfo {
                        target_fields: valid_targets.clone(),
                        match_types: HashMap::new(),
                        confidences: HashMap::new(),
                    };
                    for target in &valid_targets {
                        match_info.match_types.insert(target.clone(), MatchType::Import);
                        match_info.confidences.insert(target.clone(), 1.0);
                    }
                    matches.insert(source_field.path.clone(), match_info);
                    continue;
                }
            }

            // 3. Check exact name match
            if let Some(target_field) = target_field_lookup.get(source_field_name as &str) {
                let types_match = source_field.field_type == target_field.field_type;
                matches.insert(
                    source_field.path.clone(),
                    MatchInfo::single(
                        target_field.path.clone(),
                        if types_match { MatchType::Exact } else { MatchType::TypeMismatch(Box::new(MatchType::Exact)) },
                        if types_match { 1.0 } else { 0.7 },
                    ),
                );
                continue;
            }

            // 4. Check prefix-transformed matches (1-to-N support)
            // Skip if this field path is in negative_matches (blocks prefix matching)
            if !negative_matches.contains(&source_field.path) {
                let transformed_names = apply_prefix_transform(source_field_name, prefix_mappings);
                let mut valid_transformed = Vec::new();
                for transformed_name in transformed_names {
                    if let Some(target_field) = target_field_lookup.get(&transformed_name) {
                        let types_match = source_field.field_type == target_field.field_type;
                        valid_transformed.push((
                            target_field.path.clone(),
                            if types_match { MatchType::Prefix } else { MatchType::TypeMismatch(Box::new(MatchType::Prefix)) },
                            if types_match { 0.9 } else { 0.6 },
                        ));
                    }
                }

                if !valid_transformed.is_empty() {
                    let mut match_info = MatchInfo {
                        target_fields: valid_transformed.iter().map(|(path, _, _)| path.clone()).collect(),
                        match_types: HashMap::new(),
                        confidences: HashMap::new(),
                    };
                    for (target_path, match_type, confidence) in valid_transformed {
                        match_info.match_types.insert(target_path.clone(), match_type);
                        match_info.confidences.insert(target_path, confidence);
                    }
                    matches.insert(source_field.path.clone(), match_info);
                }
            }
        }
    }

    matches
}

/// Information about a path in the metadata tree
#[derive(Clone, Debug)]
struct PathInfo {
    path: String,
    is_field: bool,
    field_type: Option<crate::api::metadata::FieldType>,
}

/// Build paths from metadata for a specific tab type
fn build_metadata_paths(metadata: &crate::api::EntityMetadata, tab_type: &str) -> Vec<PathInfo> {
    let mut paths = Vec::new();

    match tab_type {
        "forms" => {
            // Build paths for forms
            for form in &metadata.forms {
                let formtype_path = format!("formtype/{}", form.form_type);
                // Add formtype container
                paths.push(PathInfo {
                    path: formtype_path.clone(),
                    is_field: false,
                    field_type: None,
                });

                let form_path = format!("{}/form/{}", formtype_path, form.name);
                // Add form container
                paths.push(PathInfo {
                    path: form_path.clone(),
                    is_field: false,
                    field_type: None,
                });

                if let Some(structure) = &form.form_structure {
                    for tab in &structure.tabs {
                        let tab_path = format!("{}/tab/{}", form_path, tab.name);
                        // Add tab container
                        paths.push(PathInfo {
                            path: tab_path.clone(),
                            is_field: false,
                            field_type: None,
                        });

                        for section in &tab.sections {
                            let section_path = format!("{}/section/{}", tab_path, section.name);
                            // Add section container
                            paths.push(PathInfo {
                                path: section_path.clone(),
                                is_field: false,
                                field_type: None,
                            });

                            for field in &section.fields {
                                let field_path = format!("{}/{}", section_path, field.logical_name);
                                // Add field
                                paths.push(PathInfo {
                                    path: field_path,
                                    is_field: true,
                                    field_type: Some(crate::api::metadata::FieldType::Other("FormField".to_string())),
                                });
                            }
                        }
                    }
                }
            }
        }
        "views" => {
            // Build paths for views
            for view in &metadata.views {
                let viewtype_path = format!("viewtype/{}", view.view_type);
                // Add viewtype container
                paths.push(PathInfo {
                    path: viewtype_path.clone(),
                    is_field: false,
                    field_type: None,
                });

                let view_path = format!("{}/view/{}", viewtype_path, view.name);
                // Add view container
                paths.push(PathInfo {
                    path: view_path.clone(),
                    is_field: false,
                    field_type: None,
                });

                for column in &view.columns {
                    let column_path = format!("{}/{}", view_path, column.name);
                    // Add column
                    paths.push(PathInfo {
                        path: column_path,
                        is_field: true,
                        field_type: Some(crate::api::metadata::FieldType::Other("Column".to_string())),
                    });
                }
            }
        }
        _ => {}
    }

    paths
}

/// Recompute field and relationship matches based on current mappings
/// Call this after manual mappings, imported mappings, or prefix mappings change
pub fn recompute_all_matches(
    source_metadata: &crate::api::EntityMetadata,
    target_metadata: &crate::api::EntityMetadata,
    field_mappings: &HashMap<String, Vec<String>>,
    imported_mappings: &HashMap<String, Vec<String>>,
    prefix_mappings: &HashMap<String, Vec<String>>,
    examples: &super::ExamplesState,
    source_entity: &str,
    target_entity: &str,
    negative_matches: &std::collections::HashSet<String>,
) -> (
    HashMap<String, MatchInfo>,  // field_matches
    HashMap<String, MatchInfo>,  // relationship_matches
    HashMap<String, MatchInfo>,  // entity_matches
    Vec<(String, usize)>,        // source_entities
    Vec<(String, usize)>,        // target_entities
) {
    // Flat matching for Fields tab
    let mut all_field_matches = compute_field_matches(
        &source_metadata.fields,
        &target_metadata.fields,
        field_mappings,
        imported_mappings,
        prefix_mappings,
        examples,
        source_entity,
        target_entity,
        negative_matches,
    );

    // Hierarchical matching for Forms tab
    let forms_matches = compute_hierarchical_field_matches(
        source_metadata,
        target_metadata,
        field_mappings,
        imported_mappings,
        prefix_mappings,
        "forms",
        negative_matches,
    );
    all_field_matches.extend(forms_matches);

    // Hierarchical matching for Views tab
    let views_matches = compute_hierarchical_field_matches(
        source_metadata,
        target_metadata,
        field_mappings,
        imported_mappings,
        prefix_mappings,
        "views",
        negative_matches,
    );
    all_field_matches.extend(views_matches);

    // Extract entities from relationships
    let source_entities = super::extract_entities(&source_metadata.relationships);
    let target_entities = super::extract_entities(&target_metadata.relationships);

    // Compute entity matches (uses same mappings as fields)
    let entity_matches = compute_entity_matches(
        &source_entities,
        &target_entities,
        field_mappings,
        prefix_mappings,
    );

    // Relationship matching (now entity-aware)
    let relationship_matches = compute_relationship_matches(
        &source_metadata.relationships,
        &target_metadata.relationships,
        field_mappings,
        prefix_mappings,
        &entity_matches,
    );

    (all_field_matches, relationship_matches, entity_matches, source_entities, target_entities)
}
