use crate::tui::command::Command;
use crate::tui::widgets::TreeEvent;
use super::super::{Msg, ActiveTab};
use super::super::app::State;
use super::super::tree_sync::{
    update_mirrored_selection,
    update_mirrored_navigation,
    mirror_container_toggle,
    update_reverse_mirrored_selection,
    update_reverse_mirrored_navigation,
    mirror_container_toggle_reverse,
};

pub fn handle_source_tree_event(state: &mut State, event: TreeEvent) -> Command<Msg> {
    // Handle source tree navigation/interaction
    // Note: focused_side is updated ONLY via on_focus callback (see view.rs tree builder)
    let tree_state = match state.active_tab {
        ActiveTab::Fields => &mut state.source_fields_tree,
        ActiveTab::Relationships => &mut state.source_relationships_tree,
        ActiveTab::Views => &mut state.source_views_tree,
        ActiveTab::Forms => &mut state.source_forms_tree,
        ActiveTab::Entities => &mut state.source_entities_tree,
    };

    // Check if this is a toggle event before handling
    let is_toggle = matches!(event, TreeEvent::Toggle);
    let node_id_before_toggle = if is_toggle {
        tree_state.selected().map(|s| s.to_string())
    } else {
        None
    };

    tree_state.handle_event(event);

    // Get selected ID before releasing the borrow
    let selected_id = tree_state.selected().map(|s| s.to_string());

    // Check if node is expanded (for toggle mirroring)
    let is_expanded = if let Some(id) = &node_id_before_toggle {
        tree_state.is_expanded(id)
    } else {
        false
    };

    // Release the borrow by dropping tree_state reference
    drop(tree_state);

    // Mirror container expansion/collapse only if mirror mode is Source
    if state.mirror_mode == super::super::models::MirrorMode::Source {
        if let Some(toggled_id) = node_id_before_toggle {
            mirror_container_toggle(state, &toggled_id, is_expanded);
        }

        // Mirror navigation (cursor position) to target tree WITHOUT modifying multi-selection
        // This allows the target tree to show the matched item as you navigate the source tree,
        // but selection only happens on explicit user actions (clicks, Space key for multi-select)
        if let Some(source_id) = selected_id {
            update_mirrored_navigation(state, &source_id);
        }
    }

    Command::None
}

pub fn handle_target_tree_event(state: &mut State, event: TreeEvent) -> Command<Msg> {
    // Handle target tree navigation/interaction
    // Note: focused_side is updated ONLY via on_focus callback (see view.rs tree builder)
    let tree_state = match state.active_tab {
        ActiveTab::Fields => &mut state.target_fields_tree,
        ActiveTab::Relationships => &mut state.target_relationships_tree,
        ActiveTab::Views => &mut state.target_views_tree,
        ActiveTab::Forms => &mut state.target_forms_tree,
        ActiveTab::Entities => &mut state.target_entities_tree,
    };

    // Check if this is a toggle event before handling
    let is_toggle = matches!(event, TreeEvent::Toggle);
    let node_id_before_toggle = if is_toggle {
        tree_state.selected().map(|s| s.to_string())
    } else {
        None
    };

    tree_state.handle_event(event);

    // Get selected ID before releasing the borrow
    let selected_id = tree_state.selected().map(|s| s.to_string());

    // Check if node is expanded (for toggle mirroring)
    let is_expanded = if let Some(id) = &node_id_before_toggle {
        tree_state.is_expanded(id)
    } else {
        false
    };

    // Release the borrow by dropping tree_state reference
    drop(tree_state);

    // Mirror container expansion/collapse only if mirror mode is Target
    if state.mirror_mode == super::super::models::MirrorMode::Target {
        if let Some(toggled_id) = node_id_before_toggle {
            mirror_container_toggle_reverse(state, &toggled_id, is_expanded);
        }

        // Mirror navigation (cursor position) to source tree WITHOUT modifying multi-selection
        // This allows the source tree to show the matched item as you navigate the target tree,
        // but selection only happens on explicit user actions (clicks, Space key for multi-select)
        if let Some(target_id) = selected_id {
            update_reverse_mirrored_navigation(state, &target_id);
        }
    }

    Command::None
}

pub fn handle_source_viewport_height(state: &mut State, height: usize) -> Command<Msg> {
    // Renderer calls this with actual viewport height
    let tree_state = match state.active_tab {
        ActiveTab::Fields => &mut state.source_fields_tree,
        ActiveTab::Relationships => &mut state.source_relationships_tree,
        ActiveTab::Views => &mut state.source_views_tree,
        ActiveTab::Forms => &mut state.source_forms_tree,
        ActiveTab::Entities => &mut state.source_entities_tree,
    };
    tree_state.set_viewport_height(height);
    Command::None
}

pub fn handle_target_viewport_height(state: &mut State, height: usize) -> Command<Msg> {
    // Renderer calls this with actual viewport height
    let tree_state = match state.active_tab {
        ActiveTab::Fields => &mut state.target_fields_tree,
        ActiveTab::Relationships => &mut state.target_relationships_tree,
        ActiveTab::Views => &mut state.target_views_tree,
        ActiveTab::Forms => &mut state.target_forms_tree,
        ActiveTab::Entities => &mut state.target_entities_tree,
    };
    tree_state.set_viewport_height(height);
    Command::None
}

pub fn handle_source_node_clicked(state: &mut State, node_id: String) -> Command<Msg> {
    // Note: focused_side is updated automatically via on_focus callback when tree gains focus

    // Get the tree state for the active tab
    let tree_state = match state.active_tab {
        ActiveTab::Fields => &mut state.source_fields_tree,
        ActiveTab::Relationships => &mut state.source_relationships_tree,
        ActiveTab::Views => &mut state.source_views_tree,
        ActiveTab::Forms => &mut state.source_forms_tree,
        ActiveTab::Entities => &mut state.source_entities_tree,
    };

    // Update selection and scroll to ensure visibility
    tree_state.select_and_scroll(Some(node_id.clone()));

    // Release the borrow
    drop(tree_state);

    // Trigger mirrored selection to update target tree only if mirror mode is Source
    if state.mirror_mode == super::super::models::MirrorMode::Source {
        update_mirrored_selection(state, &node_id);
    }

    Command::None
}

pub fn handle_target_node_clicked(state: &mut State, node_id: String) -> Command<Msg> {
    // Note: focused_side is updated automatically via on_focus callback when tree gains focus

    // Get the tree state for the active tab
    let tree_state = match state.active_tab {
        ActiveTab::Fields => &mut state.target_fields_tree,
        ActiveTab::Relationships => &mut state.target_relationships_tree,
        ActiveTab::Views => &mut state.target_views_tree,
        ActiveTab::Forms => &mut state.target_forms_tree,
        ActiveTab::Entities => &mut state.target_entities_tree,
    };

    // Update selection and scroll to ensure visibility
    tree_state.select_and_scroll(Some(node_id.clone()));

    // Release the borrow
    drop(tree_state);

    // Trigger reverse mirrored selection to update source tree only if mirror mode is Target
    if state.mirror_mode == super::super::models::MirrorMode::Target {
        update_reverse_mirrored_selection(state, &node_id);
    }

    Command::None
}

pub fn handle_source_tree_focused(state: &mut State) -> Command<Msg> {
    // Update focused side when source tree gains focus
    state.focused_side = super::super::Side::Source;
    Command::None
}

pub fn handle_target_tree_focused(state: &mut State) -> Command<Msg> {
    // Update focused side when target tree gains focus
    state.focused_side = super::super::Side::Target;
    Command::None
}
