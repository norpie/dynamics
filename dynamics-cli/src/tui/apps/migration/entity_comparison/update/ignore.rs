//! Ignore handlers for hiding unwanted fields/relationships/entities

use super::super::Msg;
use super::super::app::State;
use crate::tui::Command;
use crossterm::event::KeyCode;

/// Get the item identifier for currently selected item based on active tab
fn get_selected_item_id(state: &mut State) -> Option<String> {
    // Capture values before borrowing trees
    let active_tab = state.active_tab;
    let focused_side = state.focused_side;

    let active_tree = match focused_side {
        super::super::Side::Source => state.source_tree_for_tab(),
        super::super::Side::Target => state.target_tree_for_tab(),
    };

    if let Some(selected_node_id) = active_tree.selected().as_ref() {
        // Build identifier: "tab:side:node_id"
        let tab_prefix = match active_tab {
            super::super::ActiveTab::Fields => "fields",
            super::super::ActiveTab::Relationships => "relationships",
            super::super::ActiveTab::Views => "views",
            super::super::ActiveTab::Forms => "forms",
            super::super::ActiveTab::Entities => "entities",
        };
        let side_prefix = match focused_side {
            super::super::Side::Source => "source",
            super::super::Side::Target => "target",
        };
        Some(format!(
            "{}:{}:{}",
            tab_prefix, side_prefix, selected_node_id
        ))
    } else {
        None
    }
}

/// Toggle ignore state for the currently selected item
pub fn handle_ignore_item(state: &mut State) -> Command<Msg> {
    if let Some(item_id) = get_selected_item_id(state) {
        // Toggle: if already ignored, un-ignore it; otherwise ignore it
        if state.ignored_items.contains(&item_id) {
            log::info!("Un-ignoring item: {}", item_id);
            state.ignored_items.remove(&item_id);
        } else {
            log::info!("Ignoring item: {}", item_id);
            state.ignored_items.insert(item_id.clone());
        }

        // Persist to config (async, don't wait)
        // TODO: Support multi-entity mode - for now use first entity
        let source_entity = state.source_entities.first().cloned().unwrap_or_default();
        let target_entity = state.target_entities.first().cloned().unwrap_or_default();
        let ignored = state.ignored_items.clone();

        Command::perform(
            async move {
                let config = crate::global_config();
                if let Err(e) = config
                    .set_ignored_items(&source_entity, &target_entity, &ignored)
                    .await
                {
                    log::error!("Failed to save ignored items: {}", e);
                }
            },
            |_| Msg::IgnoredItemsSaved, // Dummy message - doesn't trigger another ignore
        )
    } else {
        log::warn!("No item selected to ignore");
        Command::None
    }
}

/// Open the ignore manager modal
pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_ignore_modal = true;
    let item_count = state.ignored_items.len();
    state.ignore_list_state.select_and_scroll(
        if state.ignored_items.is_empty() {
            None
        } else {
            Some(0)
        },
        item_count,
    );
    Command::None
}

/// Close the ignore manager modal
pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_ignore_modal = false;
    Command::None
}

/// Handle navigation in ignore list
pub fn handle_navigate(state: &mut State, key: KeyCode) -> Command<Msg> {
    let item_count = state.ignored_items.len();
    if item_count == 0 {
        return Command::None;
    }

    // Use approximate viewport height - the actual height is set by on_render
    state.ignore_list_state.handle_key(key, item_count, 20);
    Command::None
}

/// Handle selecting an item in ignore list
pub fn handle_select(state: &mut State, index: usize) -> Command<Msg> {
    let item_count = state.ignored_items.len();
    state
        .ignore_list_state
        .select_and_scroll(Some(index), item_count);
    Command::None
}

/// Delete the currently selected ignored item
pub fn handle_delete_item(state: &mut State) -> Command<Msg> {
    if let Some(selected_index) = state.ignore_list_state.selected() {
        let ignored_vec: Vec<String> = state.ignored_items.iter().cloned().collect();
        if selected_index < ignored_vec.len() {
            let item_to_remove = &ignored_vec[selected_index];
            log::info!("Removing ignored item: {}", item_to_remove);
            state.ignored_items.remove(item_to_remove);

            // Adjust selection after deletion
            let new_count = state.ignored_items.len();
            state.ignore_list_state.select_and_scroll(
                if new_count == 0 {
                    None
                } else if selected_index >= new_count {
                    Some(new_count - 1)
                } else {
                    Some(selected_index)
                },
                new_count,
            );

            // Persist to config
            // TODO: Support multi-entity mode - for now use first entity
            let source_entity = state.source_entities.first().cloned().unwrap_or_default();
            let target_entity = state.target_entities.first().cloned().unwrap_or_default();
            let ignored = state.ignored_items.clone();

            return Command::perform(
                async move {
                    let config = crate::global_config();
                    if let Err(e) = config
                        .set_ignored_items(&source_entity, &target_entity, &ignored)
                        .await
                    {
                        log::error!("Failed to save ignored items: {}", e);
                    }
                },
                |_| Msg::IgnoredItemsSaved, // Dummy message - doesn't close modal
            );
        }
    }
    Command::None
}

/// Clear all ignored items
pub fn handle_clear_all(state: &mut State) -> Command<Msg> {
    log::info!("Clearing all ignored items");
    state.ignored_items.clear();
    state.ignore_list_state.select_and_scroll(None, 0);

    // Persist cleared state to config
    // TODO: Support multi-entity mode - for now use first entity
    let source_entity = state.source_entities.first().cloned().unwrap_or_default();
    let target_entity = state.target_entities.first().cloned().unwrap_or_default();

    Command::perform(
        async move {
            let config = crate::global_config();
            if let Err(e) = config
                .clear_ignored_items(&source_entity, &target_entity)
                .await
            {
                log::error!("Failed to clear ignored items in config: {}", e);
            }
        },
        |_| Msg::IgnoredItemsSaved, // Dummy message - doesn't close modal
    )
}

/// Update viewport height for ignore list scrolling
pub fn handle_set_viewport_height(state: &mut State, height: usize) -> Command<Msg> {
    let item_count = state.ignored_items.len();
    state.ignore_list_state.set_viewport_height(height);
    state.ignore_list_state.update_scroll(height, item_count);
    Command::None
}
