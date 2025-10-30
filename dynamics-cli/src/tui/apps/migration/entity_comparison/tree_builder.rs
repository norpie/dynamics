//! Build tree items from entity metadata

use crate::api::EntityMetadata;
use crate::api::metadata::FieldType;
use super::tree_items::{ComparisonTreeItem, FieldNode, RelationshipNode, ViewNode, FormNode, ContainerNode, ContainerMatchType, EntityNode};
use super::ActiveTab;
use super::models::MatchInfo;
use std::collections::HashMap;

/// Build tree items for the active tab from metadata with match information
pub fn build_tree_items(
    metadata: &EntityMetadata,
    active_tab: ActiveTab,
    field_matches: &HashMap<String, MatchInfo>,
    relationship_matches: &HashMap<String, MatchInfo>,
    entity_matches: &HashMap<String, MatchInfo>,
    entities: &[(String, usize)],
    examples: &super::models::ExamplesState,
    is_source: bool,
    entity_name: &str,
    show_technical_names: bool,
    sort_mode: super::models::SortMode,
    ignored_items: &std::collections::HashSet<String>,
) -> Vec<ComparisonTreeItem> {
    let tab_prefix = match active_tab {
        ActiveTab::Fields => "fields",
        ActiveTab::Relationships => "relationships",
        ActiveTab::Views => "views",
        ActiveTab::Forms => "forms",
        ActiveTab::Entities => "entities",
    };
    let side_prefix = if is_source { "source" } else { "target" };

    match active_tab {
        ActiveTab::Fields => build_fields_tree(&metadata.fields, field_matches, examples, is_source, entity_name, show_technical_names, sort_mode, ignored_items, tab_prefix, side_prefix),
        ActiveTab::Relationships => build_relationships_tree(&metadata.relationships, relationship_matches, sort_mode, ignored_items, tab_prefix, side_prefix),
        ActiveTab::Views => build_views_tree(&metadata.views, field_matches, &metadata.fields, examples, is_source, entity_name, show_technical_names, ignored_items, tab_prefix, side_prefix),
        ActiveTab::Forms => build_forms_tree(&metadata.forms, field_matches, &metadata.fields, examples, is_source, entity_name, show_technical_names, ignored_items, tab_prefix, side_prefix),
        ActiveTab::Entities => build_entities_tree(entities, entity_matches, sort_mode, ignored_items, tab_prefix, side_prefix),
    }
}

/// Build tree items for the Fields tab
/// Note: Relationship fields are already filtered out in data_loading.rs
fn build_fields_tree(
    fields: &[crate::api::metadata::FieldMetadata],
    field_matches: &HashMap<String, MatchInfo>,
    examples: &super::models::ExamplesState,
    is_source: bool,
    entity_name: &str,
    show_technical_names: bool,
    sort_mode: super::models::SortMode,
    ignored_items: &std::collections::HashSet<String>,
    tab_prefix: &str,
    side_prefix: &str,
) -> Vec<ComparisonTreeItem> {
    let mut items: Vec<ComparisonTreeItem> = fields
        .iter()
        .map(|f| {
            // Compute display name: technical (logical) or friendly (display_name)
            let display_name = if show_technical_names {
                f.logical_name.clone()
            } else {
                f.display_name.clone().unwrap_or_else(|| f.logical_name.clone())
            };

            let item_id = format!("{}:{}:{}", tab_prefix, side_prefix, f.logical_name);
            ComparisonTreeItem::Field(FieldNode {
                metadata: f.clone(),
                match_info: field_matches.get(&f.logical_name).cloned(),
                example_value: examples.get_field_value(&f.logical_name, is_source, entity_name),
                display_name,
                is_ignored: ignored_items.contains(&item_id),
            })
        })
        .collect();

    sort_items(&mut items, sort_mode);
    items
}

/// Build tree items for the Relationships tab
fn build_relationships_tree(
    relationships: &[crate::api::metadata::RelationshipMetadata],
    relationship_matches: &HashMap<String, MatchInfo>,
    sort_mode: super::models::SortMode,
    ignored_items: &std::collections::HashSet<String>,
    tab_prefix: &str,
    side_prefix: &str,
) -> Vec<ComparisonTreeItem> {
    let mut items: Vec<ComparisonTreeItem> = relationships
        .iter()
        .map(|r| {
            // IMPORTANT: Must use "rel_{name}" to match RelationshipNode::id() in tree_items.rs
            let item_id = format!("{}:{}:rel_{}", tab_prefix, side_prefix, r.name);
            ComparisonTreeItem::Relationship(RelationshipNode {
                metadata: r.clone(),
                match_info: relationship_matches.get(&r.name).cloned(),
                is_ignored: ignored_items.contains(&item_id),
            })
        })
        .collect();

    sort_items(&mut items, sort_mode);
    items
}

/// Build tree items for the Views tab
/// Hierarchy: ViewType → View → Column (as field reference)
/// Uses path-based IDs for hierarchical matching
fn build_views_tree(
    views: &[crate::api::metadata::ViewMetadata],
    field_matches: &HashMap<String, MatchInfo>,
    all_fields: &[crate::api::metadata::FieldMetadata],
    examples: &super::models::ExamplesState,
    is_source: bool,
    entity_name: &str,
    show_technical_names: bool,
    ignored_items: &std::collections::HashSet<String>,
    tab_prefix: &str,
    side_prefix: &str,
) -> Vec<ComparisonTreeItem> {
    // Group views by type
    let mut grouped: HashMap<String, Vec<&crate::api::metadata::ViewMetadata>> = HashMap::new();
    for view in views {
        grouped.entry(view.view_type.clone()).or_default().push(view);
    }

    let mut result = Vec::new();

    // Sort keys for deterministic ordering
    let mut view_types: Vec<_> = grouped.keys().cloned().collect();
    view_types.sort();

    for view_type in view_types {
        let mut type_views = grouped.get(&view_type).unwrap().clone();
        // Sort views alphabetically within each type
        type_views.sort_by(|a, b| a.name.cmp(&b.name));

        let mut view_containers = Vec::new();

        // Build path for this view type
        let viewtype_path = format!("viewtype/{}", view_type);

        // Track name occurrences to deduplicate
        let mut name_counts: HashMap<String, usize> = HashMap::new();

        for view in type_views {
            // Build matching path using name (for cross-environment matching)
            let matching_path = format!("{}/view/{}", viewtype_path, view.name);

            // Build unique node ID (append counter if duplicate name)
            let count = name_counts.entry(view.name.clone()).or_insert(0);
            let view_path = if *count == 0 {
                matching_path.clone()
            } else {
                format!("{}#{}", matching_path, count)
            };
            *count += 1;

            // Create field nodes for each column
            let column_fields: Vec<ComparisonTreeItem> = view.columns.iter()
                .map(|col| {
                    // Build paths: matching uses name-based path for cross-environment matching
                    let matching_column_path = format!("{}/{}", matching_path, col.name);
                    let column_path = format!("{}/{}", view_path, col.name);

                    // Look up actual field metadata from entity's fields
                    let field_metadata = if let Some(real_field) = lookup_field_metadata(all_fields, &col.name) {
                        // Use real field metadata with path-based ID
                        crate::api::metadata::FieldMetadata {
                            logical_name: column_path.clone(), // Use full path as ID for matching
                            display_name: real_field.display_name.clone(),
                            field_type: real_field.field_type.clone(),
                            is_required: real_field.is_required,
                            is_primary_key: col.is_primary,
                            max_length: real_field.max_length,
                            related_entity: real_field.related_entity.clone(),
                        }
                    } else {
                        // Fallback to placeholder if field not found
                        crate::api::metadata::FieldMetadata {
                            logical_name: column_path.clone(),
                            display_name: None,
                            field_type: FieldType::Other("Column".to_string()),
                            is_required: false,
                            is_primary_key: col.is_primary,
                            max_length: None,
                            related_entity: None,
                        }
                    };

                    // Compute display name for the field
                    let display_name = if show_technical_names {
                        // For views: extract just the field name from the path
                        column_path.split('/').last().unwrap_or(&column_path).to_string()
                    } else {
                        field_metadata.display_name.clone().unwrap_or_else(|| {
                            column_path.split('/').last().unwrap_or(&column_path).to_string()
                        })
                    };

                    ComparisonTreeItem::Field(FieldNode {
                        metadata: field_metadata,
                        match_info: field_matches.get(&matching_column_path).cloned(),
                        example_value: examples.get_field_value(&matching_column_path, is_source, entity_name),
                        display_name,
                        is_ignored: false, // Views/forms columns not individually ignorable for now
                    })
                })
                .collect();

            // Create container for this view (use matching_path for lookup, view_path for node ID)
            let (container_match_type, match_info) = compute_container_match_type(&matching_path, &column_fields, field_matches);
            view_containers.push(ComparisonTreeItem::Container(ContainerNode {
                id: view_path.clone(),
                label: format!("{} ({} columns)", view.name, view.columns.len()),
                children: column_fields,
                container_match_type,
                match_info,
            }));
        }

        // Create container for this view type
        let (container_match_type, match_info) = compute_container_match_type(&viewtype_path, &view_containers, field_matches);
        result.push(ComparisonTreeItem::Container(ContainerNode {
            id: viewtype_path.clone(),
            label: format!("{} ({} views)", view_type, view_containers.len()),
            children: view_containers,
            container_match_type,
            match_info,
        }));
    }

    result
}

/// Build tree items for the Forms tab
/// Hierarchy: FormType → Form → Tab → Section → Field
/// Uses path-based IDs for hierarchical matching
fn build_forms_tree(
    forms: &[crate::api::metadata::FormMetadata],
    field_matches: &HashMap<String, MatchInfo>,
    all_fields: &[crate::api::metadata::FieldMetadata],
    examples: &super::models::ExamplesState,
    is_source: bool,
    entity_name: &str,
    show_technical_names: bool,
    ignored_items: &std::collections::HashSet<String>,
    tab_prefix: &str,
    side_prefix: &str,
) -> Vec<ComparisonTreeItem> {
    // Group forms by type
    let mut grouped: HashMap<String, Vec<&crate::api::metadata::FormMetadata>> = HashMap::new();
    for form in forms {
        grouped.entry(form.form_type.clone()).or_default().push(form);
    }

    let mut result = Vec::new();

    // Sort keys for deterministic ordering
    let mut form_types: Vec<_> = grouped.keys().cloned().collect();
    form_types.sort();

    for form_type in form_types {
        let mut type_forms = grouped.get(&form_type).unwrap().clone();
        // Sort forms alphabetically within each type
        type_forms.sort_by(|a, b| a.name.cmp(&b.name));
        let mut form_containers = Vec::new();

        // Build path for this form type
        let formtype_path = format!("formtype/{}", form_type);

        // Track name occurrences to deduplicate
        let mut name_counts: HashMap<String, usize> = HashMap::new();

        for form in type_forms {
            // Build matching path using name (for cross-environment matching)
            let matching_form_path = format!("{}/form/{}", formtype_path, form.name);

            // Build unique node ID (append counter if duplicate name)
            let count = name_counts.entry(form.name.clone()).or_insert(0);
            let form_path = if *count == 0 {
                matching_form_path.clone()
            } else {
                format!("{}#{}", matching_form_path, count)
            };
            *count += 1;

            // If form has structure, build nested hierarchy
            let form_children = if let Some(structure) = &form.form_structure {
                let mut tab_containers = Vec::new();

                // Sort tabs by order
                let mut tabs = structure.tabs.clone();
                tabs.sort_by_key(|t| t.order);

                for tab in &tabs {
                    // Build paths: matching uses name, node ID uses unique form_path
                    let matching_tab_path = format!("{}/tab/{}", matching_form_path, tab.name);
                    let tab_path = format!("{}/tab/{}", form_path, tab.name);
                    let mut section_containers = Vec::new();

                    // Sort sections by order
                    let mut sections = tab.sections.clone();
                    sections.sort_by_key(|s| s.order);

                    for section in &sections {
                        // Build paths: matching uses name, node ID uses unique tab_path
                        let matching_section_path = format!("{}/section/{}", matching_tab_path, section.name);
                        let section_path = format!("{}/section/{}", tab_path, section.name);

                        // Sort fields by row order
                        let mut fields = section.fields.clone();
                        fields.sort_by_key(|f| (f.row, f.column));

                        let field_nodes: Vec<ComparisonTreeItem> = fields.iter()
                            .map(|field| {
                                // Build paths: matching uses name-based path, node ID uses unique section_path
                                let matching_field_path = format!("{}/{}", matching_section_path, field.logical_name);
                                let field_path = format!("{}/{}", section_path, field.logical_name);

                                // Look up actual field metadata from entity's fields
                                let field_metadata = if let Some(real_field) = lookup_field_metadata(all_fields, &field.logical_name) {
                                    // Use real field metadata with path-based ID
                                    crate::api::metadata::FieldMetadata {
                                        logical_name: field_path.clone(), // Use full path as ID for matching
                                        display_name: Some(field.label.clone()), // Keep form's label
                                        field_type: real_field.field_type.clone(),
                                        is_required: field.required_level != "None",
                                        is_primary_key: real_field.is_primary_key,
                                        max_length: real_field.max_length,
                                        related_entity: real_field.related_entity.clone(),
                                    }
                                } else {
                                    // Fallback to placeholder if field not found
                                    crate::api::metadata::FieldMetadata {
                                        logical_name: field_path.clone(),
                                        display_name: Some(field.label.clone()),
                                        field_type: FieldType::Other("FormField".to_string()),
                                        is_required: field.required_level != "None",
                                        is_primary_key: false,
                                        max_length: None,
                                        related_entity: None,
                                    }
                                };

                                // Compute display name for the field
                                let display_name = if show_technical_names {
                                    // For forms: extract just the field name from the path
                                    field_path.split('/').last().unwrap_or(&field_path).to_string()
                                } else {
                                    field_metadata.display_name.clone().unwrap_or_else(|| {
                                        field_path.split('/').last().unwrap_or(&field_path).to_string()
                                    })
                                };

                                ComparisonTreeItem::Field(FieldNode {
                                    metadata: field_metadata,
                                    match_info: field_matches.get(&matching_field_path).cloned(),
                                    example_value: examples.get_field_value(&matching_field_path, is_source, entity_name),
                                    display_name,
                                    is_ignored: false, // Forms/views fields not individually ignorable for now
                                })
                            })
                            .collect();

                        // Create container for section (use matching path for lookup)
                        let (container_match_type, match_info) = compute_container_match_type(&matching_section_path, &field_nodes, field_matches);
                        section_containers.push(ComparisonTreeItem::Container(ContainerNode {
                            id: section_path.clone(),
                            label: format!("{} ({} fields)", section.label, section.fields.len()),
                            children: field_nodes,
                            container_match_type,
                            match_info,
                        }));
                    }

                    // Create container for tab (use matching path for lookup)
                    let (container_match_type, match_info) = compute_container_match_type(&matching_tab_path, &section_containers, field_matches);
                    tab_containers.push(ComparisonTreeItem::Container(ContainerNode {
                        id: tab_path.clone(),
                        label: format!("{} ({} sections)", tab.label, tab.sections.len()),
                        children: section_containers,
                        container_match_type,
                        match_info,
                    }));
                }

                tab_containers
            } else {
                // No structure available - empty form
                vec![]
            };

            // Create container for this form (use matching path for lookup)
            let (container_match_type, match_info) = compute_container_match_type(&matching_form_path, &form_children, field_matches);
            form_containers.push(ComparisonTreeItem::Container(ContainerNode {
                id: form_path.clone(),
                label: if form_children.is_empty() {
                    format!("{} (no structure)", form.name)
                } else {
                    format!("{} ({} tabs)", form.name, form_children.len())
                },
                children: form_children,
                container_match_type,
                match_info,
            }));
        }

        // Create container for this form type
        let (container_match_type, match_info) = compute_container_match_type(&formtype_path, &form_containers, field_matches);
        result.push(ComparisonTreeItem::Container(ContainerNode {
            id: formtype_path.clone(),
            label: format!("{} ({} forms)", form_type, form_containers.len()),
            children: form_containers,
            container_match_type,
            match_info,
        }));
    }

    result
}

/// Look up actual field metadata from entity's fields list by logical name
/// Returns None if field not found
fn lookup_field_metadata<'a>(
    fields: &'a [crate::api::metadata::FieldMetadata],
    logical_name: &str,
) -> Option<&'a crate::api::metadata::FieldMetadata> {
    fields.iter().find(|f| f.logical_name == logical_name)
}

/// Compute ContainerMatchType and MatchInfo based on container's own match and children's match status
///
/// Logic:
/// - NoMatch: Container path doesn't match (OR no children and not matched)
/// - FullMatch: Container path matches AND all children match
/// - Mixed: Container path matches BUT not all children match
///
/// Returns: (ContainerMatchType, Option<MatchInfo>)
fn compute_container_match_type(
    container_id: &str,
    children: &[ComparisonTreeItem],
    field_matches: &HashMap<String, MatchInfo>,
) -> (ContainerMatchType, Option<MatchInfo>) {
    // Check if this container itself has a match
    let match_info = field_matches.get(container_id).cloned();

    if match_info.is_some() {
        log::debug!("Container '{}' has match: {:?}", container_id, match_info);
    }

    if match_info.is_none() {
        // Container doesn't match → NoMatch
        return (ContainerMatchType::NoMatch, None);
    }

    // Container matched - now check children
    if children.is_empty() {
        // Container matched but has no children → FullMatch
        return (ContainerMatchType::FullMatch, match_info);
    }

    let mut has_matched = false;
    let mut has_unmatched = false;

    for child in children {
        let child_has_match = match child {
            ComparisonTreeItem::Field(node) => node.match_info.is_some(),
            ComparisonTreeItem::Relationship(node) => node.match_info.is_some(),
            ComparisonTreeItem::Container(node) => {
                // Recursively check container status
                node.container_match_type != ContainerMatchType::NoMatch
            }
            _ => false,
        };

        if child_has_match {
            has_matched = true;
        } else {
            has_unmatched = true;
        }

        // Early exit if we know it's mixed
        if has_matched && has_unmatched {
            return (ContainerMatchType::Mixed, match_info);
        }
    }

    if has_matched && !has_unmatched {
        (ContainerMatchType::FullMatch, match_info)
    } else {
        (ContainerMatchType::Mixed, match_info)  // Container matched but some/all children didn't
    }
}

/// Build tree items for the Entities tab
fn build_entities_tree(
    entities: &[(String, usize)],
    entity_matches: &HashMap<String, MatchInfo>,
    sort_mode: super::models::SortMode,
    ignored_items: &std::collections::HashSet<String>,
    tab_prefix: &str,
    side_prefix: &str,
) -> Vec<ComparisonTreeItem> {
    let mut items: Vec<ComparisonTreeItem> = entities
        .iter()
        .map(|(name, usage_count)| {
            // IMPORTANT: Must use "entity_{name}" to match EntityNode::id() in tree_items.rs
            let item_id = format!("{}:{}:entity_{}", tab_prefix, side_prefix, name);
            ComparisonTreeItem::Entity(EntityNode {
                name: name.clone(),
                match_info: entity_matches.get(name).cloned(),
                usage_count: *usage_count,
                is_ignored: ignored_items.contains(&item_id),
            })
        })
        .collect();

    sort_items(&mut items, sort_mode);
    items
}

/// Sort tree items based on sort mode
fn sort_items(items: &mut [ComparisonTreeItem], sort_mode: super::models::SortMode) {
    match sort_mode {
        super::models::SortMode::Alphabetical => {
            // Sort alphabetically by name
            items.sort_by(|a, b| {
                let a_name = item_name(a);
                let b_name = item_name(b);
                a_name.cmp(&b_name)
            });
        }
        super::models::SortMode::MatchesFirst | super::models::SortMode::SourceMatches => {
            // Sort in three tiers:
            // 1. Matched items (alphabetically)
            // 2. Unmatched items (alphabetically)
            // 3. Ignored items (alphabetically)
            // For SourceMatches, this is only applied to source side - target side uses special logic
            items.sort_by(|a, b| {
                let a_has_match = item_has_match(a);
                let b_has_match = item_has_match(b);
                let a_is_ignored = item_is_ignored(a);
                let b_is_ignored = item_is_ignored(b);

                // Determine sort tier (lower number = higher priority)
                let a_tier = if a_is_ignored {
                    2  // Ignored items last
                } else if a_has_match {
                    0  // Matched items first
                } else {
                    1  // Unmatched items in the middle
                };

                let b_tier = if b_is_ignored {
                    2
                } else if b_has_match {
                    0
                } else {
                    1
                };

                // First sort by tier
                match a_tier.cmp(&b_tier) {
                    std::cmp::Ordering::Equal => {
                        // Same tier - sort alphabetically
                        let a_name = item_name(a);
                        let b_name = item_name(b);
                        a_name.cmp(&b_name)
                    }
                    other => other,
                }
            });
        }
    }
}

/// Get the name of an item for sorting
fn item_name(item: &ComparisonTreeItem) -> &str {
    match item {
        ComparisonTreeItem::Field(node) => &node.metadata.logical_name,
        ComparisonTreeItem::Relationship(node) => &node.metadata.name,
        ComparisonTreeItem::Entity(node) => &node.name,
        ComparisonTreeItem::Container(node) => &node.label,
        _ => "",
    }
}

/// Check if an item has a match
fn item_has_match(item: &ComparisonTreeItem) -> bool {
    match item {
        ComparisonTreeItem::Field(node) => node.match_info.is_some(),
        ComparisonTreeItem::Relationship(node) => node.match_info.is_some(),
        ComparisonTreeItem::Entity(node) => node.match_info.is_some(),
        ComparisonTreeItem::Container(node) => node.match_info.is_some(),
        _ => false,
    }
}

/// Check if an item is ignored
fn item_is_ignored(item: &ComparisonTreeItem) -> bool {
    match item {
        ComparisonTreeItem::Field(node) => node.is_ignored,
        ComparisonTreeItem::Relationship(node) => node.is_ignored,
        ComparisonTreeItem::Entity(node) => node.is_ignored,
        _ => false,
    }
}
