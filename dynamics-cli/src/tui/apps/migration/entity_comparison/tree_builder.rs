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
    sort_mode: super::models::SortMode,
) -> Vec<ComparisonTreeItem> {
    match active_tab {
        ActiveTab::Fields => build_fields_tree(&metadata.fields, field_matches, examples, is_source, entity_name, sort_mode),
        ActiveTab::Relationships => build_relationships_tree(&metadata.relationships, relationship_matches, sort_mode),
        ActiveTab::Views => build_views_tree(&metadata.views, field_matches, &metadata.fields, examples, is_source, entity_name),
        ActiveTab::Forms => build_forms_tree(&metadata.forms, field_matches, &metadata.fields, examples, is_source, entity_name),
        ActiveTab::Entities => build_entities_tree(entities, entity_matches, sort_mode),
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
    sort_mode: super::models::SortMode,
) -> Vec<ComparisonTreeItem> {
    let mut items: Vec<ComparisonTreeItem> = fields
        .iter()
        .map(|f| ComparisonTreeItem::Field(FieldNode {
            metadata: f.clone(),
            match_info: field_matches.get(&f.logical_name).cloned(),
            example_value: examples.get_field_value(&f.logical_name, is_source, entity_name),
        }))
        .collect();

    sort_items(&mut items, sort_mode);
    items
}

/// Build tree items for the Relationships tab
fn build_relationships_tree(
    relationships: &[crate::api::metadata::RelationshipMetadata],
    relationship_matches: &HashMap<String, MatchInfo>,
    sort_mode: super::models::SortMode,
) -> Vec<ComparisonTreeItem> {
    let mut items: Vec<ComparisonTreeItem> = relationships
        .iter()
        .map(|r| ComparisonTreeItem::Relationship(RelationshipNode {
            metadata: r.clone(),
            match_info: relationship_matches.get(&r.name).cloned(),
        }))
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

        for view in type_views {
            // Build path for this view
            let view_path = format!("{}/view/{}", viewtype_path, view.name);

            // Create field nodes for each column
            let column_fields: Vec<ComparisonTreeItem> = view.columns.iter()
                .map(|col| {
                    // Build full path for this column
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

                    ComparisonTreeItem::Field(FieldNode {
                        metadata: field_metadata,
                        match_info: field_matches.get(&column_path).cloned(),
                        example_value: examples.get_field_value(&column_path, is_source, entity_name),
                    })
                })
                .collect();

            // Create container for this view
            let (container_match_type, match_info) = compute_container_match_type(&view_path, &column_fields, field_matches);
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

        for form in type_forms {
            // Build path for this form
            let form_path = format!("{}/form/{}", formtype_path, form.name);

            // If form has structure, build nested hierarchy
            let form_children = if let Some(structure) = &form.form_structure {
                let mut tab_containers = Vec::new();

                // Sort tabs by order
                let mut tabs = structure.tabs.clone();
                tabs.sort_by_key(|t| t.order);

                for tab in &tabs {
                    // Build path for this tab using label (not ID)
                    let tab_path = format!("{}/tab/{}", form_path, tab.label);
                    let mut section_containers = Vec::new();

                    // Sort sections by order
                    let mut sections = tab.sections.clone();
                    sections.sort_by_key(|s| s.order);

                    for section in &sections {
                        // Build path for this section using label (not ID)
                        let section_path = format!("{}/section/{}", tab_path, section.label);

                        // Sort fields by row order
                        let mut fields = section.fields.clone();
                        fields.sort_by_key(|f| (f.row, f.column));

                        let field_nodes: Vec<ComparisonTreeItem> = fields.iter()
                            .map(|field| {
                                // Build full path for this field
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

                                ComparisonTreeItem::Field(FieldNode {
                                    metadata: field_metadata,
                                    match_info: field_matches.get(&field_path).cloned(),
                                    example_value: examples.get_field_value(&field_path, is_source, entity_name),
                                })
                            })
                            .collect();

                        // Create container for section
                        let (container_match_type, match_info) = compute_container_match_type(&section_path, &field_nodes, field_matches);
                        section_containers.push(ComparisonTreeItem::Container(ContainerNode {
                            id: section_path.clone(),
                            label: format!("{} ({} fields)", section.label, section.fields.len()),
                            children: field_nodes,
                            container_match_type,
                            match_info,
                        }));
                    }

                    // Create container for tab
                    let (container_match_type, match_info) = compute_container_match_type(&tab_path, &section_containers, field_matches);
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

            // Create container for this form
            let (container_match_type, match_info) = compute_container_match_type(&form_path, &form_children, field_matches);
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
) -> Vec<ComparisonTreeItem> {
    let mut items: Vec<ComparisonTreeItem> = entities
        .iter()
        .map(|(name, usage_count)| ComparisonTreeItem::Entity(EntityNode {
            name: name.clone(),
            match_info: entity_matches.get(name).cloned(),
            usage_count: *usage_count,
        }))
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
            // Sort matched items first (alphabetically), then unmatched (alphabetically)
            // For SourceMatches, this is only applied to source side - target side uses special logic
            items.sort_by(|a, b| {
                let a_has_match = item_has_match(a);
                let b_has_match = item_has_match(b);

                match (a_has_match, b_has_match) {
                    (true, false) => std::cmp::Ordering::Less,    // Matched before unmatched
                    (false, true) => std::cmp::Ordering::Greater, // Unmatched after matched
                    _ => {
                        // Both matched or both unmatched - sort alphabetically
                        let a_name = item_name(a);
                        let b_name = item_name(b);
                        a_name.cmp(&b_name)
                    }
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
