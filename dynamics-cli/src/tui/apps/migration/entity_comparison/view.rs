use crate::tui::{
    element::Element,
    state::theme::Theme,
    Resource,
    widgets::TreeState,
    modals::ConfirmationModal,
    Alignment as LayerAlignment,
};
use crate::api::EntityMetadata;
use crate::{col, row, use_constraints};
use super::{Msg, ActiveTab};
use super::app::State;
use super::tree_builder::build_tree_items;
use super::tree_items::ComparisonTreeItem;
use std::collections::HashMap;
use super::{MatchInfo, MatchType};

/// Render the main side-by-side layout with source and target trees
pub fn render_main_layout(state: &mut State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use_constraints!();

    // Get tree items from cache (already built in update())
    // This eliminates the expensive rebuild that was happening every frame
    let active_tab = state.active_tab;
    let hide_mode = state.hide_mode;
    let sort_mode = state.sort_mode;
    let mut source_items = if let Some(ref cache) = state.tree_cache {
        cache.source_items.clone()
    } else {
        // Fallback: should never happen since update() rebuilds cache
        log::warn!("Tree cache missing in view(), using empty tree");
        vec![]
    };

    // Apply hide mode filtering
    source_items = match hide_mode {
        super::models::HideMode::Off => source_items,
        super::models::HideMode::HideMatched => filter_matched_items(source_items),
        super::models::HideMode::HideIgnored => filter_ignored_items(source_items),
        super::models::HideMode::HideMatchedAndIgnored => filter_matched_and_ignored_items(source_items),
        super::models::HideMode::HideExamples => filter_example_matches(source_items),
        super::models::HideMode::HideAll => filter_all_items(source_items),
    };

    // Apply search filter based on search mode
    let (source_search_query, target_search_query) = super::update::search::get_search_terms(state);
    let source_search_active = source_search_query.is_some();
    let target_search_active = target_search_query.is_some();

    if let Some(query) = source_search_query {
        let source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
        source_items = filter_tree_items_by_search(
            source_items,
            query,
            state.match_mode,
            &state.examples,
            true, // is_source
            source_entity,
        );
    }

    let mut target_items = if let Some(ref cache) = state.tree_cache {
        cache.target_items.clone()
    } else {
        // Fallback: should never happen since update() rebuilds cache
        log::warn!("Tree cache missing in view(), using empty tree");
        vec![]
    };

    // Apply hide mode filtering
    target_items = match hide_mode {
        super::models::HideMode::Off => target_items,
        super::models::HideMode::HideMatched => filter_matched_items(target_items),
        super::models::HideMode::HideIgnored => filter_ignored_items(target_items),
        super::models::HideMode::HideMatchedAndIgnored => filter_matched_and_ignored_items(target_items),
        super::models::HideMode::HideExamples => filter_example_matches(target_items),
        super::models::HideMode::HideAll => filter_all_items(target_items),
    };

    // Apply search filter if there's a search query
    if let Some(query) = target_search_query {
        let target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");
        target_items = filter_tree_items_by_search(
            target_items,
            query,
            state.match_mode,
            &state.examples,
            false, // is_source
            target_entity,
        );
    }

    // Apply type filtering based on type filter mode
    let (source_type_filter, target_type_filter) = match state.type_filter_mode {
        super::models::TypeFilterMode::Unified => {
            (&state.unified_type_filter, &state.unified_type_filter)
        }
        super::models::TypeFilterMode::Independent => {
            (&state.source_type_filter, &state.target_type_filter)
        }
    };

    let source_type_filter_active = source_type_filter.is_some();
    let target_type_filter_active = target_type_filter.is_some();

    if let Some(filter_type) = source_type_filter {
        source_items = filter_tree_items_by_type(source_items, filter_type);
    }

    if let Some(filter_type) = target_type_filter {
        target_items = filter_tree_items_by_type(target_items, filter_type);
    }

    // Apply SourceMatches sorting to target side if enabled
    if sort_mode == super::models::SortMode::SourceMatches {
        target_items = sort_target_by_source_order(&source_items, target_items, state.sort_direction);
    }

    // Calculate detailed completion statistics
    let (source_stats, target_stats) = calculate_detailed_completion_stats(state, active_tab);

    // Cache entity names before borrowing tree states
    let source_entity_name = state.source_entities.first().cloned().unwrap_or_default();
    let target_entity_name = state.target_entities.first().cloned().unwrap_or_default();

    // Get the appropriate tree state for the active tab based on which side
    let (source_tree_state, target_tree_state) = match active_tab {
        ActiveTab::Fields => (&mut state.source_fields_tree, &mut state.target_fields_tree),
        ActiveTab::Relationships => (&mut state.source_relationships_tree, &mut state.target_relationships_tree),
        ActiveTab::Views => (&mut state.source_views_tree, &mut state.target_views_tree),
        ActiveTab::Forms => (&mut state.source_forms_tree, &mut state.target_forms_tree),
        ActiveTab::Entities => (&mut state.source_entities_tree, &mut state.target_entities_tree),
    };

    // Auto-expand containers with matching children when searching or type filtering
    // This ensures filtered children are visible even if containers were collapsed
    if source_search_active || source_type_filter_active {
        auto_expand_containers_with_children(&source_items, source_tree_state);
        // Note: Scroll reset moved to update handlers (only when filter changes)
    }

    if target_search_active || target_type_filter_active {
        auto_expand_containers_with_children(&target_items, target_tree_state);
        // Note: Scroll reset moved to update handlers (only when filter changes)
    }

    // Invalidate tree cache when search, type filter, or hide mode filtering is active
    // This ensures visible_order reflects the actual filtered items
    if source_search_active || source_type_filter_active || hide_mode != super::models::HideMode::Off {
        source_tree_state.invalidate_cache();
    }
    if target_search_active || target_type_filter_active || hide_mode != super::models::HideMode::Off {
        target_tree_state.invalidate_cache();
    }

    // Source panel with tree - renderer will call on_render with actual area.height
    let source_panel_title = {
        let multi_select_count = source_tree_state.total_selection_count();
        let stats_str = source_stats.format_compact();
        if multi_select_count > 1 {
            format!("Source: {} ({}) - {} selected", source_entity_name, stats_str, multi_select_count)
        } else {
            format!("Source: {} ({})", source_entity_name, stats_str)
        }
    };

    let source_panel = Element::panel(
        Element::tree("source_tree", &source_items, source_tree_state, theme)
            .on_event(Msg::SourceTreeEvent)
            .on_select(Msg::SourceTreeNodeClicked)
            .on_render(Msg::SourceViewportHeight)
            .on_focus(Msg::SourceTreeFocused)
            .build()
    )
    .title(source_panel_title)
    .build();

    // Target panel with tree - renderer will call on_render with actual area.height
    let target_panel = Element::panel(
        Element::tree("target_tree", &target_items, target_tree_state, theme)
            .on_event(Msg::TargetTreeEvent)
            .on_select(Msg::TargetTreeNodeClicked)
            .on_render(Msg::TargetViewportHeight)
            .on_focus(Msg::TargetTreeFocused)
            .build()
    )
    .title(format!("Target: {} ({})", target_entity_name, target_stats.format_compact()))
    .build();

    // Count filtered results
    let source_count = source_items.len();
    let target_count = target_items.len();

    // Create search UI based on mode
    use super::models::SearchMode;
    let search_ui = match state.search_mode {
        SearchMode::Unified => {
            // Single search box that filters both sides, wrapped in a panel
            let search_input = Element::text_input(
                crate::tui::FocusId::new("unified-search-input"),
                state.unified_search.value(),
                &state.unified_search.state,
            )
            .placeholder("Search both sides... (/ to focus, ? to split)")
            .on_event(Msg::SearchInputEvent)
            .on_blur(Msg::SearchInputBlur)
            .build();

            let title = if source_search_active {
                format!("Search (Unified) - {} matches in source, {} matches in target", source_count, target_count)
            } else {
                "Search (Unified)".to_string()
            };

            Element::panel(search_input)
                .title(title)
                .build()
        }
        SearchMode::Independent => {
            // Two search boxes side by side, each in its own panel
            let source_search = Element::text_input(
                crate::tui::FocusId::new("source-search-input"),
                state.source_search.value(),
                &state.source_search.state,
            )
            .placeholder("Filter source... (? to merge)")
            .on_event(Msg::SourceSearchEvent)
            .on_blur(Msg::SourceSearchBlur)
            .build();

            let target_search = Element::text_input(
                crate::tui::FocusId::new("target-search-input"),
                state.target_search.value(),
                &state.target_search.state,
            )
            .placeholder("Filter target...")
            .on_event(Msg::TargetSearchEvent)
            .on_blur(Msg::TargetSearchBlur)
            .build();

            // Wrap each in a panel with title showing match count
            let source_panel_title = if source_search_active {
                format!("Source Search ({} matches)", source_count)
            } else {
                "Source Search".to_string()
            };

            let target_panel_title = if target_search_active {
                format!("Target Search ({} matches)", target_count)
            } else {
                "Target Search".to_string()
            };

            let source_panel = Element::panel(source_search)
                .title(source_panel_title)
                .build();

            let target_panel = Element::panel(target_search)
                .title(target_panel_title)
                .build();

            // Return row of two panels (no outer wrapper)
            row![
                source_panel => Fill(1),
                target_panel => Fill(1),
            ]
        }
    };

    // Create type filter UI based on mode
    use super::models::{TypeFilterMode, format_type_filter};
    let type_filter_ui = match state.type_filter_mode {
        TypeFilterMode::Unified => {
            let filter_text = format_type_filter(&state.unified_type_filter);
            let title = format!("Type Filter (Unified): {}", filter_text);
            Element::panel(Element::text(&title))
                .title("Type Filter")
                .build()
        }
        TypeFilterMode::Independent => {
            let source_filter_text = format_type_filter(&state.source_type_filter);
            let target_filter_text = format_type_filter(&state.target_type_filter);

            let source_panel = Element::panel(Element::text(&format!("Type: {}", source_filter_text)))
                .title("Source Type")
                .build();

            let target_panel = Element::panel(Element::text(&format!("Type: {}", target_filter_text)))
                .title("Target Type")
                .build();

            row![
                source_panel => Fill(1),
                target_panel => Fill(1),
            ]
        }
    };

    // Main layout with search and type filter
    // Note: Both search modes use 3 lines (1 panel with input = 3 lines height)
    // Type filter uses 3 lines as well
    col![
        search_ui => Length(3),
        type_filter_ui => Length(3),
        row![
            source_panel => Fill(1),
            target_panel => Fill(1),
        ] => Fill(1),
    ]
}

/// Render the back confirmation modal
pub fn render_back_confirmation_modal() -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    ConfirmationModal::new("Go Back?")
        .message("Are you sure you want to go back to the comparison list?")
        .confirm_text("Yes")
        .cancel_text("No")
        .on_confirm(Msg::ConfirmBack)
        .on_cancel(Msg::CancelBack)
        .width(60)
        .height(10)
        .build()
}

pub fn render_examples_modal(state: &State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::modals::{ExamplesModal, ExamplePairItem};

    // Convert example pairs to list items
    let pair_items: Vec<ExamplePairItem<Msg>> = state.examples.pairs.iter().map(|pair| {
        ExamplePairItem {
            id: pair.id.clone(),
            source_id: pair.source_record_id.clone(),
            target_id: pair.target_record_id.clone(),
            label: pair.label.clone(),
            on_delete: Msg::DeleteExamplePair,
        }
    }).collect();

    ExamplesModal::new()
        .pairs(pair_items)
        .source_input_state(state.examples_source_input.clone())
        .target_input_state(state.examples_target_input.clone())
        .label_input_state(state.examples_label_input.clone())
        .list_state(state.examples_list_state.clone())
        .on_source_input_event(Msg::SourceInputEvent)
        .on_target_input_event(Msg::TargetInputEvent)
        .on_label_input_event(Msg::LabelInputEvent)
        .on_list_navigate(Msg::ExamplesListNavigate)
        .on_list_select(Msg::ExamplesListSelect)
        .on_add(Msg::AddExamplePair)
        .on_delete(Msg::DeleteExamplePair)
        .on_close(Msg::CloseExamplesModal)
        .build()
}

pub fn render_prefix_mappings_modal(state: &State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::modals::{PrefixMappingsModal, PrefixMappingItem};

    // Convert prefix mappings to list items (join multiple targets with comma)
    let mapping_items: Vec<PrefixMappingItem<Msg>> = state.prefix_mappings.iter().map(|(source, targets)| {
        PrefixMappingItem {
            source_prefix: source.clone(),
            target_prefix: targets.join(", "),
            on_delete: Msg::DeletePrefixMapping,
        }
    }).collect();

    PrefixMappingsModal::new()
        .mappings(mapping_items)
        .source_input_state(state.prefix_source_input.clone())
        .target_input_state(state.prefix_target_input.clone())
        .list_state(state.prefix_mappings_list_state.clone())
        .on_source_input_event(Msg::PrefixSourceInputEvent)
        .on_target_input_event(Msg::PrefixTargetInputEvent)
        .on_list_navigate(Msg::PrefixMappingsListNavigate)
        .on_list_select(Msg::PrefixMappingsListSelect)
        .on_add(Msg::AddPrefixMapping)
        .on_delete(Msg::DeletePrefixMapping)
        .on_close(Msg::ClosePrefixMappingsModal)
        .build()
}

pub fn render_negative_matches_modal(state: &State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::modals::{NegativeMatchesModal, NegativeMatchItem};

    // Convert negative matches to sorted list items
    let mut negative_items: Vec<NegativeMatchItem<Msg>> = state.negative_matches.iter().map(|source_field| {
        NegativeMatchItem {
            source_field: source_field.clone(),
            on_delete: Msg::DeleteNegativeMatch,
        }
    }).collect();

    // Sort alphabetically for consistent display
    negative_items.sort_by(|a, b| a.source_field.cmp(&b.source_field));

    NegativeMatchesModal::new()
        .matches(negative_items)
        .list_state(state.negative_matches_list_state.clone())
        .on_list_navigate(Msg::NegativeMatchesListNavigate)
        .on_list_select(Msg::NegativeMatchesListSelect)
        .on_delete(Msg::DeleteNegativeMatch)
        .on_close(Msg::CloseNegativeMatchesModal)
        .build()
}

/// Filter out matched items from tree (hide unmatched, show matched)
/// Exception: ExampleValue matches are treated as unmatched (shown)
pub fn filter_matched_items(items: Vec<super::tree_items::ComparisonTreeItem>) -> Vec<super::tree_items::ComparisonTreeItem> {
    use super::tree_items::ComparisonTreeItem;
    use super::MatchType;

    items.into_iter().filter_map(|item| {
        match item {
            ComparisonTreeItem::Field(ref node) => {
                // Keep if no match OR if any target has ExampleValue match type
                if node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false) {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Relationship(ref node) => {
                // Keep if no match OR if any target has ExampleValue match type
                if node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false) {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Entity(ref node) => {
                // Keep if no match OR if any target has ExampleValue match type
                if node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false) {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_matched_items(node.children.clone());

                // Keep container if it has any unmatched/example children OR if container itself is unmatched/example
                let keep_container = node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false);

                if !filtered_children.is_empty() || keep_container {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id,
                        label: node.label,
                        children: filtered_children,
                        container_match_type: node.container_match_type,
                        match_info: node.match_info,
                    }))
                } else {
                    None
                }
            }
            // Keep View and Form nodes (they don't have match info)
            ComparisonTreeItem::View(_) | ComparisonTreeItem::Form(_) => Some(item),
        }
    }).collect()
}

/// Filter out ignored items
pub fn filter_ignored_items(items: Vec<super::tree_items::ComparisonTreeItem>) -> Vec<super::tree_items::ComparisonTreeItem> {
    use super::tree_items::ComparisonTreeItem;

    items.into_iter().filter_map(|item| {
        match item {
            ComparisonTreeItem::Field(ref node) => {
                // Keep if not ignored
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Relationship(ref node) => {
                // Keep if not ignored
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Entity(ref node) => {
                // Keep if not ignored
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_ignored_items(node.children.clone());

                // Keep container if it has any non-ignored children
                if !filtered_children.is_empty() {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id,
                        label: node.label,
                        children: filtered_children,
                        container_match_type: node.container_match_type,
                        match_info: node.match_info,
                    }))
                } else {
                    None
                }
            }
            ComparisonTreeItem::View(ref node) => {
                // Keep if not ignored
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Form(ref node) => {
                // Keep if not ignored
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
        }
    }).collect()
}

/// Filter out both matched (except examples) AND ignored items
pub fn filter_matched_and_ignored_items(items: Vec<super::tree_items::ComparisonTreeItem>) -> Vec<super::tree_items::ComparisonTreeItem> {
    use super::tree_items::ComparisonTreeItem;
    use super::MatchType;

    items.into_iter().filter_map(|item| {
        match item {
            ComparisonTreeItem::Field(ref node) => {
                // Keep if not ignored AND (no match OR any target has ExampleValue match type)
                let is_unmatched_or_example = node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false);
                if !node.is_ignored && is_unmatched_or_example {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Relationship(ref node) => {
                // Keep if not ignored AND (no match OR any target has ExampleValue match type)
                let is_unmatched_or_example = node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false);
                if !node.is_ignored && is_unmatched_or_example {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Entity(ref node) => {
                // Keep if not ignored AND (no match OR any target has ExampleValue match type)
                let is_unmatched_or_example = node.match_info.is_none()
                    || node.match_info.as_ref().map(|m| m.match_types.values().any(|t| t == &MatchType::ExampleValue)).unwrap_or(false);
                if !node.is_ignored && is_unmatched_or_example {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_matched_and_ignored_items(node.children.clone());

                // Keep container if it has any children that are not ignored and (not matched or example)
                if !filtered_children.is_empty() {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id,
                        label: node.label,
                        children: filtered_children,
                        container_match_type: node.container_match_type,
                        match_info: node.match_info,
                    }))
                } else {
                    None
                }
            }
            ComparisonTreeItem::View(ref node) => {
                // Keep if not ignored (Views don't have match info)
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Form(ref node) => {
                // Keep if not ignored (Forms don't have match info)
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
        }
    }).collect()
}

/// Filter out all matched AND ignored items (including examples)
pub fn filter_all_items(items: Vec<super::tree_items::ComparisonTreeItem>) -> Vec<super::tree_items::ComparisonTreeItem> {
    use super::tree_items::ComparisonTreeItem;

    items.into_iter().filter_map(|item| {
        match item {
            ComparisonTreeItem::Field(ref node) => {
                // Keep if not ignored AND no match
                if !node.is_ignored && node.match_info.is_none() {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Relationship(ref node) => {
                // Keep if not ignored AND no match
                if !node.is_ignored && node.match_info.is_none() {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Entity(ref node) => {
                // Keep if not ignored AND no match
                if !node.is_ignored && node.match_info.is_none() {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_all_items(node.children.clone());

                // Keep container if it has any children that are not ignored and not matched
                if !filtered_children.is_empty() {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id,
                        label: node.label,
                        children: filtered_children,
                        container_match_type: node.container_match_type,
                        match_info: node.match_info,
                    }))
                } else {
                    None
                }
            }
            ComparisonTreeItem::View(ref node) => {
                // Keep if not ignored (Views don't have match info)
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Form(ref node) => {
                // Keep if not ignored (Forms don't have match info)
                if !node.is_ignored {
                    Some(item)
                } else {
                    None
                }
            }
        }
    }).collect()
}

/// Filter out example matches (hide example matches, show everything else)
pub fn filter_example_matches(items: Vec<super::tree_items::ComparisonTreeItem>) -> Vec<super::tree_items::ComparisonTreeItem> {
    use super::tree_items::ComparisonTreeItem;
    use super::MatchType;

    items.into_iter().filter_map(|item| {
        match item {
            ComparisonTreeItem::Field(ref node) => {
                // Hide if any target has ExampleValue match type
                if let Some(ref match_info) = node.match_info {
                    if match_info.match_types.values().any(|t| t == &MatchType::ExampleValue) {
                        return None;
                    }
                }
                Some(item)
            }
            ComparisonTreeItem::Relationship(ref node) => {
                // Hide if any target has ExampleValue match type
                if let Some(ref match_info) = node.match_info {
                    if match_info.match_types.values().any(|t| t == &MatchType::ExampleValue) {
                        return None;
                    }
                }
                Some(item)
            }
            ComparisonTreeItem::Entity(ref node) => {
                // Hide if any target has ExampleValue match type
                if let Some(ref match_info) = node.match_info {
                    if match_info.match_types.values().any(|t| t == &MatchType::ExampleValue) {
                        return None;
                    }
                }
                Some(item)
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_example_matches(node.children.clone());

                // Hide container if any target has ExampleValue match type (but keep if has non-example children)
                let hide_container = if let Some(ref match_info) = node.match_info {
                    match_info.match_types.values().any(|t| t == &MatchType::ExampleValue)
                } else {
                    false
                };

                if !filtered_children.is_empty() || !hide_container {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id,
                        label: node.label,
                        children: filtered_children,
                        container_match_type: node.container_match_type,
                        match_info: node.match_info,
                    }))
                } else {
                    None
                }
            }
            // Keep View and Form nodes (they don't have match info)
            ComparisonTreeItem::View(_) | ComparisonTreeItem::Form(_) => Some(item),
        }
    }).collect()
}

/// Sort target items to align with source order in SourceMatches mode
/// Matched target items appear at the same index as their source match
/// Unmatched target items are appended alphabetically in the middle
/// Ignored target items are appended alphabetically at the end
fn sort_target_by_source_order(
    source_items: &[ComparisonTreeItem],
    mut target_items: Vec<ComparisonTreeItem>,
    sort_direction: super::models::SortDirection,
) -> Vec<ComparisonTreeItem> {
    use std::collections::HashMap;

    // Build a map of target item names to the items themselves
    let mut target_by_name: HashMap<String, ComparisonTreeItem> = target_items
        .iter()
        .map(|item| (get_item_name(item).to_string(), item.clone()))
        .collect();

    let mut result = Vec::new();
    let mut used_targets = std::collections::HashSet::new();

    // First pass: Add target items in source order (for matched items)
    for source_item in source_items {
        // Get the target field name from the source item's match
        if let Some(target_name) = get_item_match_target(source_item) {
            // Only add if not already used (prevents duplicates when multiple sources map to same target)
            if !used_targets.contains(target_name) {
                // Find and add the corresponding target item
                if let Some(target_item) = target_by_name.get(target_name) {
                    result.push(target_item.clone());
                    used_targets.insert(target_name.clone());
                }
            }
        }
    }

    // Second pass: Separate remaining items into unmatched and ignored
    let (mut ignored, mut unmatched): (Vec<ComparisonTreeItem>, Vec<ComparisonTreeItem>) = target_items
        .into_iter()
        .filter(|item| !used_targets.contains(get_item_name(item)))
        .partition(|item| get_item_is_ignored(item));

    // Sort both groups alphabetically, respecting direction
    unmatched.sort_by(|a, b| {
        let a_name = get_item_name(a);
        let b_name = get_item_name(b);
        let ordering = a_name.cmp(&b_name);
        match sort_direction {
            super::models::SortDirection::Ascending => ordering,
            super::models::SortDirection::Descending => ordering.reverse(),
        }
    });

    ignored.sort_by(|a, b| {
        let a_name = get_item_name(a);
        let b_name = get_item_name(b);
        let ordering = a_name.cmp(&b_name);
        match sort_direction {
            super::models::SortDirection::Ascending => ordering,
            super::models::SortDirection::Descending => ordering.reverse(),
        }
    });

    // Add unmatched first, then ignored
    result.extend(unmatched);
    result.extend(ignored);
    result
}

/// Get the name of an item
fn get_item_name(item: &ComparisonTreeItem) -> &str {
    match item {
        ComparisonTreeItem::Field(node) => &node.metadata.logical_name,
        ComparisonTreeItem::Relationship(node) => &node.metadata.name,
        ComparisonTreeItem::Entity(node) => &node.name,
        ComparisonTreeItem::Container(node) => &node.id,
        _ => "",
    }
}

/// Get the primary target field name from an item's match info
fn get_item_match_target(item: &ComparisonTreeItem) -> Option<&str> {
    match item {
        ComparisonTreeItem::Field(node) => node.match_info.as_ref().and_then(|m| m.primary_target()).map(|s| s.as_str()),
        ComparisonTreeItem::Relationship(node) => node.match_info.as_ref().and_then(|m| m.primary_target()).map(|s| s.as_str()),
        ComparisonTreeItem::Entity(node) => node.match_info.as_ref().and_then(|m| m.primary_target()).map(|s| s.as_str()),
        ComparisonTreeItem::Container(node) => node.match_info.as_ref().and_then(|m| m.primary_target()).map(|s| s.as_str()),
        _ => None,
    }
}

/// Check if an item is ignored
fn get_item_is_ignored(item: &ComparisonTreeItem) -> bool {
    match item {
        ComparisonTreeItem::Field(node) => node.is_ignored,
        ComparisonTreeItem::Relationship(node) => node.is_ignored,
        ComparisonTreeItem::Entity(node) => node.is_ignored,
        _ => false,
    }
}

pub fn render_manual_mappings_modal(state: &State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::modals::{ManualMappingsModal, ManualMappingItem};

    // Convert manual field mappings to list items (join multiple targets with comma)
    let mapping_items: Vec<ManualMappingItem<Msg>> = state.field_mappings.iter().map(|(source, targets)| {
        ManualMappingItem {
            source_field: source.clone(),
            target_field: targets.join(", "),
            on_delete: Msg::DeleteManualMappingFromModal,
        }
    }).collect();

    ManualMappingsModal::new()
        .mappings(mapping_items)
        .list_state(state.manual_mappings_list_state.clone())
        .on_list_navigate(Msg::ManualMappingsListNavigate)
        .on_list_select(Msg::ManualMappingsListSelect)
        .on_delete(Msg::DeleteManualMappingFromModal)
        .on_close(Msg::CloseManualMappingsModal)
        .build()
}

/// Render import results modal showing what was added/removed/couldn't be parsed
pub fn render_import_results_modal(state: &mut State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::element::LayoutConstraint::*;
    use crate::{col, spacer, button_row};
    use ratatui::text::{Line, Span};
    use ratatui::style::{Style, Stylize};
    use crate::tui::widgets::ListItem;

    let results = state.import_results.as_ref().unwrap();

    // Build list items - using String instead of Line to avoid lifetime issues
    #[derive(Clone)]
    struct ImportResultLine {
        text: String,
        style: Style,
    }

    impl ListItem for ImportResultLine {
        type Msg = Msg;

        fn to_element(&self, _is_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
            Element::styled_text(Line::from(Span::styled(self.text.clone(), self.style))).build()
        }
    }

    let mut list_items: Vec<ImportResultLine> = vec![];

    // Header line
    list_items.push(ImportResultLine {
        text: format!("Import Results: {}", results.filename),
        style: Style::default().fg(theme.accent_primary).bold(),
    });
    list_items.push(ImportResultLine {
        text: String::new(),
        style: Style::default(),
    });

    // Added mappings
    if !results.added.is_empty() {
        list_items.push(ImportResultLine {
            text: format!("✓ Added {} mappings", results.added.len()),
            style: Style::default().fg(theme.accent_success).bold(),
        });
        for (src, tgt) in &results.added {
            list_items.push(ImportResultLine {
                text: format!("  {} → {}", src, tgt),
                style: Style::default().fg(theme.text_primary),
            });
        }
        list_items.push(ImportResultLine {
            text: String::new(),
            style: Style::default(),
        });
    }

    // Updated mappings
    if !results.updated.is_empty() {
        list_items.push(ImportResultLine {
            text: format!("⟳ Updated {} mappings", results.updated.len()),
            style: Style::default().fg(theme.accent_warning).bold(),
        });
        for (src, tgt) in &results.updated {
            list_items.push(ImportResultLine {
                text: format!("  {} → {}", src, tgt),
                style: Style::default().fg(theme.text_primary),
            });
        }
        list_items.push(ImportResultLine {
            text: String::new(),
            style: Style::default(),
        });
    }

    // Removed mappings
    if !results.removed.is_empty() {
        list_items.push(ImportResultLine {
            text: format!("✗ Removed {} mappings", results.removed.len()),
            style: Style::default().fg(theme.accent_error).bold(),
        });
        for (src, tgt) in &results.removed {
            list_items.push(ImportResultLine {
                text: format!("  {} → {}", src, tgt),
                style: Style::default().fg(theme.text_secondary),
            });
        }
        list_items.push(ImportResultLine {
            text: String::new(),
            style: Style::default(),
        });
    }

    // Unparsed lines
    if !results.unparsed.is_empty() {
        list_items.push(ImportResultLine {
            text: format!("⚠ Couldn't parse {} lines", results.unparsed.len()),
            style: Style::default().fg(theme.accent_warning).bold(),
        });
        for line in &results.unparsed {
            let truncated = if line.len() > 60 {
                format!("{}...", &line[..57])
            } else {
                line.clone()
            };
            list_items.push(ImportResultLine {
                text: format!("  {}", truncated),
                style: Style::default().fg(theme.text_tertiary),
            });
        }
    }

    // If no changes at all, show a message
    if results.added.is_empty() && results.updated.is_empty() && results.removed.is_empty() && results.unparsed.is_empty() {
        list_items.push(ImportResultLine {
            text: "No changes detected".to_string(),
            style: Style::default().fg(theme.text_tertiary).italic(),
        });
    }

    // List
    log::debug!("Import results modal: {} list items", list_items.len());
    let list = Element::list("import-results-list", &list_items, &state.import_results_list, theme)
        .on_select(Msg::ImportResultsSelect)
        .on_navigate(Msg::ImportResultsNavigate)
        .on_render(Msg::ImportResultsSetViewportHeight)
        .build();

    let list_panel = Element::panel(list)
        .title("Results")
        .build();

    // Buttons
    let buttons = button_row![
        ("import-results-clear", "Clear Imports (c)", Msg::ClearImportedMappings),
        ("import-results-close", "Close (Esc)", Msg::CloseImportResultsModal),
    ];

    // Layout
    let content = col![
        list_panel => Fill(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ];

    Element::panel(Element::container(content).padding(2).build())
        .title("Import Results")
        .width(90)
        .height(35)
        .build()
}

/// Render the C# mapping import modal with file browser
pub fn render_import_modal(state: &mut State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::element::LayoutConstraint::*;
    use crate::{spacer, button_row};
    use ratatui::text::{Line, Span};
    use ratatui::style::{Style, Stylize};

    // File browser
    let browser = Element::file_browser("import-file-browser", &state.import_file_browser, theme)
        .on_file_selected(Msg::ImportFileSelected)
        .on_navigate(Msg::ImportNavigate)
        .on_render(Msg::ImportSetViewportHeight)
        .build();

    let browser_panel = Element::panel(browser)
        .title(format!("Select C# Mapping File - {}", state.import_file_browser.current_path().display()))
        .build();

    // Help text
    let help_text = Element::styled_text(Line::from(vec![
        Span::styled("Select a .cs file to import field mappings. ", Style::default().fg(theme.text_tertiary)),
        Span::styled("Navigate with ", Style::default().fg(theme.text_tertiary)),
        Span::styled("↑/↓", Style::default().fg(theme.accent_primary).bold()),
        Span::styled(", press ", Style::default().fg(theme.text_tertiary)),
        Span::styled("Enter", Style::default().fg(theme.accent_primary).bold()),
        Span::styled(" to select.", Style::default().fg(theme.text_tertiary)),
    ])).build();

    // Info about current import
    let import_info = if let Some(ref file) = state.import_source_file {
        Element::styled_text(Line::from(vec![
            Span::styled("Currently imported: ", Style::default().fg(theme.text_secondary)),
            Span::styled(file.clone(), Style::default().fg(theme.accent_success).bold()),
            Span::styled(format!(" ({} mappings)", state.imported_mappings.len()), Style::default().fg(theme.text_tertiary)),
        ])).build()
    } else {
        Element::styled_text(Line::from(vec![
            Span::styled("No mappings currently imported", Style::default().fg(theme.text_tertiary).italic()),
        ])).build()
    };

    // Buttons
    let buttons = button_row![
        ("import-close", "Close (Esc)", Msg::CloseImportModal),
    ];

    // Layout
    let content = col![
        help_text => Length(1),
        spacer!() => Length(1),
        import_info => Length(1),
        spacer!() => Length(1),
        browser_panel => Fill(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ];

    Element::panel(Element::container(content).padding(2).build())
        .title("Import C# Field Mappings")
        .width(90)
        .height(35)
        .build()
}

/// Render the ignore manager modal
pub fn render_ignore_modal(state: &mut State) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::element::LayoutConstraint::*;
    use crate::{col, spacer, button_row};
    use ratatui::text::{Line, Span};
    use ratatui::style::{Style, Stylize};
    use crate::tui::widgets::ListItem;

    // Convert HashSet to Vec for consistent ordering
    let ignored_items: Vec<String> = state.ignored_items.iter().cloned().collect();

    // Build list items
    #[derive(Clone)]
    struct IgnoredItemLine {
        text: String,
        style: Style,
    }

    impl ListItem for IgnoredItemLine {
        type Msg = Msg;

        fn to_element(&self, is_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
            let style = if is_selected {
                self.style.bg(crate::global_runtime_config().theme.bg_surface)
            } else {
                self.style
            };
            Element::styled_text(Line::from(Span::styled(self.text.clone(), style))).build()
        }
    }

    let list_items: Vec<IgnoredItemLine> = ignored_items.iter()
        .map(|item| {
            // Parse item ID: "tab:side:node_id"
            let parts: Vec<&str> = item.split(':').collect();
            let display = if parts.len() == 3 {
                format!("[{}/{}] {}", parts[0], parts[1], parts[2])
            } else {
                item.clone()
            };

            IgnoredItemLine {
                text: display,
                style: Style::default().fg(theme.text_primary),
            }
        })
        .collect();

    // Help text
    let help_text = if ignored_items.is_empty() {
        Element::styled_text(Line::from(vec![
            Span::styled("No ignored items. ", Style::default().fg(theme.text_secondary)),
            Span::styled("Press ", Style::default().fg(theme.text_secondary)),
            Span::styled("Esc", Style::default().fg(theme.accent_primary).bold()),
            Span::styled(" to close.", Style::default().fg(theme.text_secondary)),
        ])).build()
    } else {
        Element::styled_text(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(theme.accent_primary).bold()),
            Span::styled(" Navigate  ", Style::default().fg(theme.text_secondary)),
            Span::styled("d", Style::default().fg(theme.accent_primary).bold()),
            Span::styled(" Delete  ", Style::default().fg(theme.text_secondary)),
            Span::styled("C", Style::default().fg(theme.accent_primary).bold()),
            Span::styled(" Clear All  ", Style::default().fg(theme.text_secondary)),
            Span::styled("Esc", Style::default().fg(theme.accent_primary).bold()),
            Span::styled(" Close", Style::default().fg(theme.text_secondary)),
        ])).build()
    };

    // List panel
    let list_panel = Element::list(
        "ignore-list",
        &list_items,
        &state.ignore_list_state,
        theme,
    )
    .on_render(|height| Msg::IgnoreSetViewportHeight(height))
    .build();

    // Buttons
    let buttons = button_row![
        ("ignore-delete", "Delete (d)", Msg::DeleteIgnoredItem),
        ("ignore-clear", "Clear All (C)", Msg::ClearAllIgnored),
        ("ignore-close", "Close (Esc)", Msg::CloseIgnoreModal),
    ];

    // Count info
    let count_info = Element::styled_text(Line::from(vec![
        Span::styled("Total ignored items: ", Style::default().fg(theme.text_secondary)),
        Span::styled(ignored_items.len().to_string(), Style::default().fg(theme.accent_primary).bold()),
    ])).build();

    // Layout
    let content = col![
        help_text => Length(1),
        spacer!() => Length(1),
        count_info => Length(1),
        spacer!() => Length(1),
        list_panel => Fill(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ];

    Element::panel(Element::container(content).padding(2).build())
        .title("Ignore Manager")
        .width(80)
        .height(30)
        .build()
}

/// Auto-expand containers that have children (after filtering)
/// This ensures that filtered children are visible even if the container was previously collapsed
fn auto_expand_containers_with_children(
    items: &[super::tree_items::ComparisonTreeItem],
    tree_state: &mut TreeState,
) {
    use super::tree_items::ComparisonTreeItem;

    for item in items {
        match item {
            ComparisonTreeItem::Container(node) => {
                // If container has children, expand it
                if !node.children.is_empty() {
                    tree_state.expand(&node.id);
                    // Recursively expand nested containers
                    auto_expand_containers_with_children(&node.children, tree_state);
                }
            }
            _ => {}
        }
    }
}

/// Filter tree items based on search query using the selected match mode
/// Searches logical names, display names, and example values (if enabled)
fn filter_tree_items_by_search(
    items: Vec<super::tree_items::ComparisonTreeItem>,
    query: &str,
    match_mode: super::models::MatchMode,
    examples: &super::models::ExamplesState,
    is_source: bool,
    entity_name: &str,
) -> Vec<super::tree_items::ComparisonTreeItem> {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;
    use super::tree_items::ComparisonTreeItem;

    if query.is_empty() {
        return items;
    }

    // Create matcher based on mode
    let fuzzy_matcher = if matches!(match_mode, super::models::MatchMode::Fuzzy) {
        Some(SkimMatcherV2::default())
    } else {
        None
    };
    let query_lower = query.to_lowercase();

    // Helper function to check if text matches query based on mode
    let text_matches = |text: &str| -> bool {
        match match_mode {
            super::models::MatchMode::Fuzzy => {
                fuzzy_matcher.as_ref().unwrap().fuzzy_match(text, query).is_some()
            }
            super::models::MatchMode::Substring => {
                text.to_lowercase().contains(&query_lower)
            }
        }
    };

    items.into_iter().filter_map(|item| {
        match &item {
            ComparisonTreeItem::Field(node) => {
                // Search technical name (logical_name)
                let logical_match = text_matches(&node.metadata.logical_name);

                // Search friendly name (display_name from metadata) if it exists
                // This ensures we search both names regardless of current display mode
                let display_match = node.metadata.display_name
                    .as_ref()
                    .map(|name| text_matches(name))
                    .unwrap_or(false);

                // Search example value if examples are enabled
                let example_match = if examples.enabled {
                    examples.get_field_value(&node.metadata.logical_name, is_source, entity_name)
                        .map(|value| text_matches(&value))
                        .unwrap_or(false)
                } else {
                    false
                };

                if logical_match || display_match || example_match {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Relationship(node) => {
                // Search relationship name and related entity
                let name_match = text_matches(&node.metadata.name);
                let entity_match = text_matches(&node.metadata.related_entity);

                if name_match || entity_match {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Entity(node) => {
                // Search entity name
                if text_matches(&node.name) {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_tree_items_by_search(
                    node.children.clone(),
                    query,
                    match_mode,
                    examples,
                    is_source,
                    entity_name,
                );

                // Keep container if it has matching children OR if container label matches
                let label_match = text_matches(&node.label);

                if !filtered_children.is_empty() || label_match {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id.clone(),
                        label: node.label.clone(),
                        children: filtered_children,
                        container_match_type: node.container_match_type.clone(),
                        match_info: node.match_info.clone(),
                    }))
                } else {
                    None
                }
            }
            ComparisonTreeItem::View(node) => {
                // Search view name
                if text_matches(&node.metadata.name) {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Form(node) => {
                // Search form name
                if text_matches(&node.metadata.name) {
                    Some(item)
                } else {
                    None
                }
            }
        }
    }).collect()
}

/// Filter tree items by field type
/// Only filters Field items, keeps all other item types (Containers, Views, Forms, etc.)
fn filter_tree_items_by_type(
    items: Vec<super::tree_items::ComparisonTreeItem>,
    filter_type: &crate::api::metadata::models::FieldType,
) -> Vec<super::tree_items::ComparisonTreeItem> {
    use super::tree_items::ComparisonTreeItem;

    items.into_iter().filter_map(|item| {
        match &item {
            ComparisonTreeItem::Field(node) => {
                // Only keep fields matching the filter type
                if &node.metadata.field_type == filter_type {
                    Some(item)
                } else {
                    None
                }
            }
            ComparisonTreeItem::Container(node) => {
                // Recursively filter container children
                let filtered_children = filter_tree_items_by_type(
                    node.children.clone(),
                    filter_type,
                );

                // Keep container if it has matching children
                if !filtered_children.is_empty() {
                    Some(ComparisonTreeItem::Container(super::tree_items::ContainerNode {
                        id: node.id.clone(),
                        label: node.label.clone(),
                        children: filtered_children,
                        container_match_type: node.container_match_type.clone(),
                        match_info: node.match_info.clone(),
                    }))
                } else {
                    None
                }
            }
            // Keep all non-field items as-is (Views, Forms, Relationships, Entities)
            // Type filtering only applies to Field items
            ComparisonTreeItem::View(_) |
            ComparisonTreeItem::Form(_) |
            ComparisonTreeItem::Relationship(_) |
            ComparisonTreeItem::Entity(_) => Some(item),
        }
    }).collect()
}

/// Detailed completion statistics for a single side (source or target)
#[derive(Debug, Clone)]
struct CompletionStats {
    total_count: usize,
    mapped_count: usize,
    ignored_count: usize,
    unhandled_count: usize,
    mapped_percentage: usize,
    ignored_percentage: usize,
    unhandled_percentage: usize,
}

impl CompletionStats {
    fn new(total: usize, mapped: usize, ignored: usize) -> Self {
        let unhandled = total.saturating_sub(mapped + ignored);
        let mapped_pct = if total > 0 {
            (mapped as f64 / total as f64 * 100.0) as usize
        } else {
            0
        };
        let ignored_pct = if total > 0 {
            (ignored as f64 / total as f64 * 100.0) as usize
        } else {
            0
        };
        let unhandled_pct = if total > 0 {
            (unhandled as f64 / total as f64 * 100.0) as usize
        } else {
            0
        };

        Self {
            total_count: total,
            mapped_count: mapped,
            ignored_count: ignored,
            unhandled_count: unhandled,
            mapped_percentage: mapped_pct,
            ignored_percentage: ignored_pct,
            unhandled_percentage: unhandled_pct,
        }
    }

    /// Format stats as a compact string for display in panel title
    /// Format: "75% | M:60% (45) I:15% (11) U:25% (19)"
    /// Where: total handled% | Mapped:% (count) Ignored:% (count) Unhandled:% (count)
    fn format_compact(&self) -> String {
        format!(
            "{}% | M:{}% ({}) I:{}% ({}) U:{}% ({})",
            self.mapped_percentage + self.ignored_percentage, // Total handled percentage
            self.mapped_percentage, self.mapped_count,
            self.ignored_percentage, self.ignored_count,
            self.unhandled_percentage, self.unhandled_count
        )
    }
}

/// Calculate detailed completion statistics for source and target sides
/// Returns (source_stats, target_stats)
fn calculate_detailed_completion_stats(state: &State, active_tab: ActiveTab) -> (CompletionStats, CompletionStats) {
    // Determine if we're in multi-entity mode (qualified names like "entity.field")
    let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

    match active_tab {
        ActiveTab::Fields => {
            // Get total counts from metadata - sum across all entities
            let source_total: usize = state.source_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.fields.len()),
                    _ => None,
                })
                .sum();

            let target_total: usize = state.target_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.fields.len()),
                    _ => None,
                })
                .sum();

            // Count source mapped fields - collect all field names from all entities
            let all_source_field_names: std::collections::HashSet<String> = state.source_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.fields.iter()
                        .map(|f| f.logical_name.clone())
                        .collect::<Vec<_>>()),
                    _ => None,
                })
                .flatten()
                .collect();

            let source_mapped = state.field_matches.keys()
                .filter(|field| {
                    // In multi-entity mode, strip entity prefix before comparing
                    let field_name = if is_multi_entity {
                        field.split('.').last().unwrap_or(field)
                    } else {
                        field.as_str()
                    };
                    all_source_field_names.contains(field_name)
                })
                .count();

            // Count source ignored fields
            let source_ignored = state.ignored_items.iter()
                .filter(|id| {
                    if let Some(field_name) = id.strip_prefix("fields:source:") {
                        // In multi-entity mode, strip entity prefix before comparing
                        let stripped_name = if is_multi_entity {
                            field_name.split('.').last().unwrap_or(field_name)
                        } else {
                            field_name
                        };
                        all_source_field_names.contains(stripped_name)
                    } else {
                        false
                    }
                })
                .count();

            // Count target mapped fields - collect all field names from all entities
            let all_target_field_names: std::collections::HashSet<String> = state.target_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.fields.iter()
                        .map(|f| f.logical_name.clone())
                        .collect::<Vec<_>>()),
                    _ => None,
                })
                .flatten()
                .collect();

            let target_mapped = state.field_matches.values()
                .flat_map(|m| m.target_fields.iter())
                .filter(|field| {
                    // In multi-entity mode, strip entity prefix before comparing
                    let field_name = if is_multi_entity {
                        field.split('.').last().unwrap_or(field)
                    } else {
                        field.as_str()
                    };
                    all_target_field_names.contains(field_name)
                })
                .collect::<std::collections::HashSet<_>>()
                .len();

            // Count target ignored fields
            let target_ignored = state.ignored_items.iter()
                .filter(|id| {
                    if let Some(field_name) = id.strip_prefix("fields:target:") {
                        // In multi-entity mode, strip entity prefix before comparing
                        let stripped_name = if is_multi_entity {
                            field_name.split('.').last().unwrap_or(field_name)
                        } else {
                            field_name
                        };
                        all_target_field_names.contains(stripped_name)
                    } else {
                        false
                    }
                })
                .count();

            (
                CompletionStats::new(source_total, source_mapped, source_ignored),
                CompletionStats::new(target_total, target_mapped, target_ignored),
            )
        }
        ActiveTab::Relationships => {
            // Get total counts from metadata - sum across all entities
            let source_total: usize = state.source_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.relationships.len()),
                    _ => None,
                })
                .sum();

            let target_total: usize = state.target_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.relationships.len()),
                    _ => None,
                })
                .sum();

            // Count source mapped relationships - collect all relationship names from all entities
            let all_source_rel_names: std::collections::HashSet<String> = state.source_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.relationships.iter()
                        .map(|r| r.name.clone())
                        .collect::<Vec<_>>()),
                    _ => None,
                })
                .flatten()
                .collect();

            let source_mapped = state.relationship_matches.keys()
                .filter(|rel| {
                    // In multi-entity mode, strip entity prefix before comparing
                    let rel_name = if is_multi_entity {
                        rel.split('.').last().unwrap_or(rel)
                    } else {
                        rel.as_str()
                    };
                    all_source_rel_names.contains(rel_name)
                })
                .count();

            // Count source ignored relationships
            let source_ignored = state.ignored_items.iter()
                .filter(|id| {
                    if let Some(rel_name) = id.strip_prefix("relationships:source:") {
                        // In multi-entity mode, strip entity prefix before comparing
                        let stripped_name = if is_multi_entity {
                            rel_name.split('.').last().unwrap_or(rel_name)
                        } else {
                            rel_name
                        };
                        all_source_rel_names.contains(stripped_name)
                    } else {
                        false
                    }
                })
                .count();

            // Count target mapped relationships - collect all relationship names from all entities
            let all_target_rel_names: std::collections::HashSet<String> = state.target_metadata.values()
                .filter_map(|r| match r {
                    Resource::Success(metadata) => Some(metadata.relationships.iter()
                        .map(|r| r.name.clone())
                        .collect::<Vec<_>>()),
                    _ => None,
                })
                .flatten()
                .collect();

            let target_mapped = state.relationship_matches.values()
                .flat_map(|m| m.target_fields.iter())
                .filter(|rel| {
                    // In multi-entity mode, strip entity prefix before comparing
                    let rel_name = if is_multi_entity {
                        rel.split('.').last().unwrap_or(rel)
                    } else {
                        rel.as_str()
                    };
                    all_target_rel_names.contains(rel_name)
                })
                .collect::<std::collections::HashSet<_>>()
                .len();

            // Count target ignored relationships
            let target_ignored = state.ignored_items.iter()
                .filter(|id| {
                    if let Some(rel_name) = id.strip_prefix("relationships:target:") {
                        // In multi-entity mode, strip entity prefix before comparing
                        let stripped_name = if is_multi_entity {
                            rel_name.split('.').last().unwrap_or(rel_name)
                        } else {
                            rel_name
                        };
                        all_target_rel_names.contains(stripped_name)
                    } else {
                        false
                    }
                })
                .count();

            (
                CompletionStats::new(source_total, source_mapped, source_ignored),
                CompletionStats::new(target_total, target_mapped, target_ignored),
            )
        }
        ActiveTab::Entities => {
            // Get total counts from entity lists
            // Use related_entities (extracted from relationships) for Entities tab stats
            let source_total = state.source_related_entities.len();
            let target_total = state.target_related_entities.len();

            // Build entity name sets for validation
            let source_entity_names: std::collections::HashSet<_> = state.source_related_entities.iter()
                .map(|(name, _)| name.as_str())
                .collect();
            let target_entity_names: std::collections::HashSet<_> = state.target_related_entities.iter()
                .map(|(name, _)| name.as_str())
                .collect();

            // Count source mapped entities - only count if entity exists in list
            let source_mapped = state.entity_matches.keys()
                .filter(|entity| source_entity_names.contains(entity.as_str()))
                .count();

            // Count source ignored entities - only count if entity exists in list
            let source_ignored = state.ignored_items.iter()
                .filter(|id| {
                    if let Some(entity_name) = id.strip_prefix("entities:source:") {
                        source_entity_names.contains(entity_name)
                    } else {
                        false
                    }
                })
                .count();

            // Count target mapped entities - only count if entity exists in list (flatten 1-to-N mappings)
            let target_mapped = state.entity_matches.values()
                .flat_map(|m| m.target_fields.iter())
                .filter(|entity| target_entity_names.contains(entity.as_str()))
                .collect::<std::collections::HashSet<_>>()
                .len();

            // Count target ignored entities - only count if entity exists in list
            let target_ignored = state.ignored_items.iter()
                .filter(|id| {
                    if let Some(entity_name) = id.strip_prefix("entities:target:") {
                        target_entity_names.contains(entity_name)
                    } else {
                        false
                    }
                })
                .count();

            (
                CompletionStats::new(source_total, source_mapped, source_ignored),
                CompletionStats::new(target_total, target_mapped, target_ignored),
            )
        }
        ActiveTab::Views | ActiveTab::Forms => {
            // Views and Forms don't have mappings/matches
            (CompletionStats::new(0, 0, 0), CompletionStats::new(0, 0, 0))
        }
    }
}
