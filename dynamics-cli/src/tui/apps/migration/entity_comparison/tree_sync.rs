use super::ActiveTab;
use super::app::State;
use crate::tui::widgets::TreeState;

/// Update target tree navigation to mirror source tree navigation (without selection)
/// Only updates the navigation cursor, does NOT modify multi-selection
pub fn update_mirrored_navigation(state: &mut State, source_id: &str) {
    // Extract the key to lookup in match maps (strip prefixes for relationships/entities)
    let source_key = match state.active_tab {
        ActiveTab::Fields => source_id.to_string(),
        ActiveTab::Relationships => source_id
            .strip_prefix("rel_")
            .unwrap_or(source_id)
            .to_string(),
        ActiveTab::Entities => source_id
            .strip_prefix("entity_")
            .unwrap_or(source_id)
            .to_string(),
        ActiveTab::Forms | ActiveTab::Views => {
            // For hierarchical tabs, use full path as key
            source_id.to_string()
        }
    };

    // Lookup matched target node IDs based on active tab (supports 1-to-N mappings)
    let target_ids: Vec<String> = match state.active_tab {
        ActiveTab::Fields | ActiveTab::Forms | ActiveTab::Views => {
            // Use field_matches for Fields and hierarchical tabs
            state
                .field_matches
                .get(&source_key)
                .map(|m| m.target_fields.clone())
                .unwrap_or_default()
        }
        ActiveTab::Relationships => {
            // Use relationship_matches
            state
                .relationship_matches
                .get(&source_key)
                .map(|m| {
                    // Add back the "rel_" prefix for target tree ID
                    m.target_fields
                        .iter()
                        .map(|tf| format!("rel_{}", tf))
                        .collect()
                })
                .unwrap_or_default()
        }
        ActiveTab::Entities => {
            // Use entity_matches
            state
                .entity_matches
                .get(&source_key)
                .map(|m| {
                    // Add back the "entity_" prefix for target tree ID
                    m.target_fields
                        .iter()
                        .map(|tf| format!("entity_{}", tf))
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    // Update target tree NAVIGATION (not selection) if matches exist
    if !target_ids.is_empty() {
        // Check if we need to expand hierarchical paths before getting mutable borrow
        let is_hierarchical = matches!(state.active_tab, ActiveTab::Forms | ActiveTab::Views);

        let target_tree = state.target_tree_for_tab();

        // For hierarchical tabs (Forms/Views), expand all parent containers for each target
        if is_hierarchical {
            for target_id in &target_ids {
                expand_parent_path(target_tree, target_id);
            }
        }

        // Navigate to first target WITHOUT modifying multi-selection
        if let Some(first_target) = target_ids.first() {
            target_tree.select_and_scroll(Some(first_target.clone()));
        }
    }
}

/// Update target tree selection to mirror source tree selection
/// Only updates if source item has a match in the target tree
pub fn update_mirrored_selection(state: &mut State, source_id: &str) {
    // Extract the key to lookup in match maps (strip prefixes for relationships/entities)
    let source_key = match state.active_tab {
        ActiveTab::Fields => source_id.to_string(),
        ActiveTab::Relationships => source_id
            .strip_prefix("rel_")
            .unwrap_or(source_id)
            .to_string(),
        ActiveTab::Entities => source_id
            .strip_prefix("entity_")
            .unwrap_or(source_id)
            .to_string(),
        ActiveTab::Forms | ActiveTab::Views => {
            // For hierarchical tabs, use full path as key
            source_id.to_string()
        }
    };

    // Lookup matched target node IDs based on active tab (supports 1-to-N mappings)
    let target_ids: Vec<String> = match state.active_tab {
        ActiveTab::Fields | ActiveTab::Forms | ActiveTab::Views => {
            // Use field_matches for Fields and hierarchical tabs
            state
                .field_matches
                .get(&source_key)
                .map(|m| m.target_fields.clone())
                .unwrap_or_default()
        }
        ActiveTab::Relationships => {
            // Use relationship_matches
            state
                .relationship_matches
                .get(&source_key)
                .map(|m| {
                    // Add back the "rel_" prefix for target tree ID
                    m.target_fields
                        .iter()
                        .map(|tf| format!("rel_{}", tf))
                        .collect()
                })
                .unwrap_or_default()
        }
        ActiveTab::Entities => {
            // Use entity_matches
            state
                .entity_matches
                .get(&source_key)
                .map(|m| {
                    // Add back the "entity_" prefix for target tree ID
                    m.target_fields
                        .iter()
                        .map(|tf| format!("entity_{}", tf))
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    // Update target tree selection if matches exist
    if !target_ids.is_empty() {
        // Check if we need to expand hierarchical paths before getting mutable borrow
        let is_hierarchical = matches!(state.active_tab, ActiveTab::Forms | ActiveTab::Views);

        let target_tree = state.target_tree_for_tab();

        // For hierarchical tabs (Forms/Views), expand all parent containers for each target
        if is_hierarchical {
            for target_id in &target_ids {
                expand_parent_path(target_tree, target_id);
            }
        }

        // Multi-select all matched targets
        target_tree.clear_multi_selection();
        for target_id in &target_ids {
            // Only toggle if not already multi-selected to avoid removing it
            if !target_tree.is_multi_selected(target_id) {
                target_tree.toggle_multi_select(target_id.clone());
            }
        }

        // Set primary selection to first target and scroll to ensure it's visible
        if let Some(first_target) = target_ids.first() {
            target_tree.select_and_scroll(Some(first_target.clone()));
        }
    }
}

/// Expand all parent containers in a path (for Forms/Views hierarchical trees)
/// Example: for path "formtype/main/form/MainForm/tab/General/fieldname"
/// Expands: "formtype/main", "formtype/main/form/MainForm", "formtype/main/form/MainForm/tab/General"
pub fn expand_parent_path(tree_state: &mut TreeState, path: &str) {
    let segments: Vec<&str> = path.split('/').collect();

    // Build each parent path and expand it
    for i in 1..segments.len() {
        let parent_path = segments[..i].join("/");
        tree_state.expand(&parent_path);
    }
}

/// Mirror container expansion/collapse from source to target tree
/// When user toggles a container in source, apply same toggle to matched container in target
pub fn mirror_container_toggle(state: &mut State, source_id: &str, is_expanded: bool) {
    // Extract the key to lookup in match maps (same logic as update_mirrored_selection)
    let source_key = match state.active_tab {
        ActiveTab::Fields => source_id.to_string(),
        ActiveTab::Relationships => source_id
            .strip_prefix("rel_")
            .unwrap_or(source_id)
            .to_string(),
        ActiveTab::Entities => source_id
            .strip_prefix("entity_")
            .unwrap_or(source_id)
            .to_string(),
        ActiveTab::Forms | ActiveTab::Views => {
            // For hierarchical tabs, use full path as key
            source_id.to_string()
        }
    };

    // Lookup matched target node IDs based on active tab (supports 1-to-N mappings)
    let target_ids: Vec<String> = match state.active_tab {
        ActiveTab::Fields | ActiveTab::Forms | ActiveTab::Views => {
            // Use field_matches for Fields and hierarchical tabs
            state
                .field_matches
                .get(&source_key)
                .map(|m| m.target_fields.clone())
                .unwrap_or_default()
        }
        ActiveTab::Relationships => {
            // Use relationship_matches
            state
                .relationship_matches
                .get(&source_key)
                .map(|m| {
                    // Add back the "rel_" prefix for target tree ID
                    m.target_fields
                        .iter()
                        .map(|tf| format!("rel_{}", tf))
                        .collect()
                })
                .unwrap_or_default()
        }
        ActiveTab::Entities => {
            // Use entity_matches
            state
                .entity_matches
                .get(&source_key)
                .map(|m| {
                    // Add back the "entity_" prefix for target tree ID
                    m.target_fields
                        .iter()
                        .map(|tf| format!("entity_{}", tf))
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    // Toggle target containers if matches exist
    if !target_ids.is_empty() {
        let target_tree = state.target_tree_for_tab();

        // Match the expansion state from source for all matched targets
        for target_id in target_ids {
            if is_expanded {
                target_tree.expand(&target_id);
            } else {
                target_tree.collapse(&target_id);
            }
        }
    }
}

// ============================================================================
// REVERSE MIRRORING FUNCTIONS (Target → Source)
// ============================================================================

/// Update source tree navigation to mirror target tree navigation (without selection)
/// Only updates the navigation cursor, does NOT modify multi-selection
pub fn update_reverse_mirrored_navigation(state: &mut State, target_id: &str) {
    // Extract the key to lookup in match maps (strip prefixes for relationships/entities)
    let target_key = match state.active_tab {
        ActiveTab::Fields => target_id.to_string(),
        ActiveTab::Relationships => target_id
            .strip_prefix("rel_")
            .unwrap_or(target_id)
            .to_string(),
        ActiveTab::Entities => target_id
            .strip_prefix("entity_")
            .unwrap_or(target_id)
            .to_string(),
        ActiveTab::Forms | ActiveTab::Views => {
            // For hierarchical tabs, use full path as key
            target_id.to_string()
        }
    };

    // Reverse lookup: find source keys that map to this target key
    let source_ids: Vec<String> = match state.active_tab {
        ActiveTab::Fields | ActiveTab::Forms | ActiveTab::Views => {
            // Search field_matches for sources that map to this target
            state
                .field_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| source_key.clone())
                .collect()
        }
        ActiveTab::Relationships => {
            // Search relationship_matches and add "rel_" prefix back
            state
                .relationship_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| format!("rel_{}", source_key))
                .collect()
        }
        ActiveTab::Entities => {
            // Search entity_matches and add "entity_" prefix back
            state
                .entity_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| format!("entity_{}", source_key))
                .collect()
        }
    };

    // Update source tree NAVIGATION (not selection) if matches exist
    if !source_ids.is_empty() {
        // Check if we need to expand hierarchical paths before getting mutable borrow
        let is_hierarchical = matches!(state.active_tab, ActiveTab::Forms | ActiveTab::Views);

        let source_tree = state.source_tree_for_tab();

        // For hierarchical tabs (Forms/Views), expand all parent containers for each source
        if is_hierarchical {
            for source_id in &source_ids {
                expand_parent_path(source_tree, source_id);
            }
        }

        // Navigate to first source WITHOUT modifying multi-selection
        if let Some(first_source) = source_ids.first() {
            source_tree.select_and_scroll(Some(first_source.clone()));
        }
    }
}

/// Update source tree selection to mirror target tree selection
/// Only updates if target item has a match in the source tree
pub fn update_reverse_mirrored_selection(state: &mut State, target_id: &str) {
    // Extract the key to lookup in match maps (strip prefixes for relationships/entities)
    let target_key = match state.active_tab {
        ActiveTab::Fields => target_id.to_string(),
        ActiveTab::Relationships => target_id
            .strip_prefix("rel_")
            .unwrap_or(target_id)
            .to_string(),
        ActiveTab::Entities => target_id
            .strip_prefix("entity_")
            .unwrap_or(target_id)
            .to_string(),
        ActiveTab::Forms | ActiveTab::Views => {
            // For hierarchical tabs, use full path as key
            target_id.to_string()
        }
    };

    // Reverse lookup: find source keys that map to this target key
    let source_ids: Vec<String> = match state.active_tab {
        ActiveTab::Fields | ActiveTab::Forms | ActiveTab::Views => {
            // Search field_matches for sources that map to this target
            state
                .field_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| source_key.clone())
                .collect()
        }
        ActiveTab::Relationships => {
            // Search relationship_matches and add "rel_" prefix back
            state
                .relationship_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| format!("rel_{}", source_key))
                .collect()
        }
        ActiveTab::Entities => {
            // Search entity_matches and add "entity_" prefix back
            state
                .entity_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| format!("entity_{}", source_key))
                .collect()
        }
    };

    // Update source tree selection if matches exist
    if !source_ids.is_empty() {
        // Check if we need to expand hierarchical paths before getting mutable borrow
        let is_hierarchical = matches!(state.active_tab, ActiveTab::Forms | ActiveTab::Views);

        let source_tree = state.source_tree_for_tab();

        // For hierarchical tabs (Forms/Views), expand all parent containers for each source
        if is_hierarchical {
            for source_id in &source_ids {
                expand_parent_path(source_tree, source_id);
            }
        }

        // Multi-select all matched sources
        source_tree.clear_multi_selection();
        for source_id in &source_ids {
            // Only toggle if not already multi-selected to avoid removing it
            if !source_tree.is_multi_selected(source_id) {
                source_tree.toggle_multi_select(source_id.clone());
            }
        }

        // Set primary selection to first source and scroll to ensure it's visible
        if let Some(first_source) = source_ids.first() {
            source_tree.select_and_scroll(Some(first_source.clone()));
        }
    }
}

/// Mirror container expansion/collapse from target to source tree
/// When user toggles a container in target, apply same toggle to matched container in source
pub fn mirror_container_toggle_reverse(state: &mut State, target_id: &str, is_expanded: bool) {
    // Extract the key to lookup in match maps
    let target_key = match state.active_tab {
        ActiveTab::Fields => target_id.to_string(),
        ActiveTab::Relationships => target_id
            .strip_prefix("rel_")
            .unwrap_or(target_id)
            .to_string(),
        ActiveTab::Entities => target_id
            .strip_prefix("entity_")
            .unwrap_or(target_id)
            .to_string(),
        ActiveTab::Forms | ActiveTab::Views => {
            // For hierarchical tabs, use full path as key
            target_id.to_string()
        }
    };

    // Reverse lookup: find source keys that map to this target key
    let source_ids: Vec<String> = match state.active_tab {
        ActiveTab::Fields | ActiveTab::Forms | ActiveTab::Views => {
            // Search field_matches for sources that map to this target
            state
                .field_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| source_key.clone())
                .collect()
        }
        ActiveTab::Relationships => {
            // Search relationship_matches and add "rel_" prefix back
            state
                .relationship_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| format!("rel_{}", source_key))
                .collect()
        }
        ActiveTab::Entities => {
            // Search entity_matches and add "entity_" prefix back
            state
                .entity_matches
                .iter()
                .filter(|(_, m)| m.target_fields.contains(&target_key))
                .map(|(source_key, _)| format!("entity_{}", source_key))
                .collect()
        }
    };

    // Toggle source containers if matches exist
    if !source_ids.is_empty() {
        let source_tree = state.source_tree_for_tab();

        // Match the expansion state from target for all matched sources
        for source_id in source_ids {
            if is_expanded {
                source_tree.expand(&source_id);
            } else {
                source_tree.collapse(&source_id);
            }
        }
    }
}
