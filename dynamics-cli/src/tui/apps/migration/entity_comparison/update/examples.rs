use crate::tui::command::Command;
use crate::tui::widgets::TextInputEvent;
use crossterm::event::KeyCode;
use super::super::{Msg, ExamplePair};
use super::super::app::State;

pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_examples_modal = true;
    Command::None
}

pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_examples_modal = false;
    Command::None
}

pub fn handle_list_navigate(state: &mut State, key: KeyCode) -> Command<Msg> {
    state.examples_list_state.handle_key(key, state.examples.pairs.len(), 10);
    Command::None
}

pub fn handle_list_select(state: &mut State, index: usize) -> Command<Msg> {
    let item_count = state.examples.pairs.len();
    state.examples_list_state.select_and_scroll(Some(index), item_count);
    Command::None
}

pub fn handle_source_input_event(state: &mut State, event: TextInputEvent) -> Command<Msg> {
    state.examples_source_input.handle_event(event, None);
    Command::None
}

pub fn handle_target_input_event(state: &mut State, event: TextInputEvent) -> Command<Msg> {
    state.examples_target_input.handle_event(event, None);
    Command::None
}

pub fn handle_label_input_event(state: &mut State, event: TextInputEvent) -> Command<Msg> {
    state.examples_label_input.handle_event(event, None);
    Command::None
}

pub fn handle_add_example_pair(state: &mut State) -> Command<Msg> {
    // Create new example pair from inputs
    let source_id = state.examples_source_input.value().trim().to_string();
    let target_id = state.examples_target_input.value().trim().to_string();
    let label = state.examples_label_input.value().trim().to_string();

    if !source_id.is_empty() && !target_id.is_empty() {
        let mut pair = ExamplePair::new(source_id, target_id);
        if !label.is_empty() {
            pair = pair.with_label(label);
        }

        let pair_id = pair.id.clone();
        let source_record_id = pair.source_record_id.clone();
        let target_record_id = pair.target_record_id.clone();

        state.examples.pairs.push(pair.clone());

        // Clear inputs
        state.examples_source_input.set_value(String::new());
        state.examples_target_input.set_value(String::new());
        state.examples_label_input.set_value(String::new());

        // Persist to database
        let source_entity = state.source_entity.clone();
        let target_entity = state.target_entity.clone();
        tokio::spawn(async move {
            let config = crate::global_config();
            if let Err(e) = config.save_example_pair(&source_entity, &target_entity, &pair).await {
                log::error!("Failed to save example pair: {}", e);
            }
        });

        // Auto-fetch data for new pair
        let source_env = state.source_env.clone();
        let source_entity = state.source_entity.clone();
        let target_env = state.target_env.clone();
        let target_entity = state.target_entity.clone();

        return Command::perform(
            async move {
                super::super::fetch_example_pair_data(
                    &source_env,
                    &source_entity,
                    &source_record_id,
                    &target_env,
                    &target_entity,
                    &target_record_id,
                ).await.map(|(source, target)| (pair_id, source, target))
            },
            |result| match result {
                Ok((pair_id, source, target)) => Msg::ExampleDataFetched(pair_id, Ok((source, target))),
                Err(e) => Msg::ExampleDataFetched(String::new(), Err(e)),
            }
        );
    }

    Command::None
}

pub fn handle_delete_example_pair(state: &mut State) -> Command<Msg> {
    // Delete selected pair from list
    if let Some(selected_idx) = state.examples_list_state.selected() {
        if selected_idx < state.examples.pairs.len() {
            let pair = state.examples.pairs.remove(selected_idx);

            // Persist to database
            let pair_id = pair.id.clone();
            tokio::spawn(async move {
                let config = crate::global_config();
                if let Err(e) = config.delete_example_pair(&pair_id).await {
                    log::error!("Failed to delete example pair: {}", e);
                }
            });
        }
    }
    Command::None
}

pub fn handle_example_data_fetched(
    state: &mut State,
    pair_id: String,
    result: Result<(serde_json::Value, serde_json::Value), String>
) -> Command<Msg> {
    // Store fetched data in cache
    match result {
        Ok((source_data, target_data)) => {
            // Find the pair and store its record IDs as cache keys
            if let Some(pair) = state.examples.pairs.iter().find(|p| p.id == pair_id) {
                log::info!("Fetched example data for pair {}: source_id={}, target_id={}",
                    pair_id, pair.source_record_id, pair.target_record_id);
                log::debug!("Source data keys: {:?}", source_data.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                log::debug!("Target data keys: {:?}", target_data.as_object().map(|o| o.keys().collect::<Vec<_>>()));

                state.examples.cache.insert(pair.source_record_id.clone(), source_data);
                state.examples.cache.insert(pair.target_record_id.clone(), target_data);
                log::info!("Cached example data for pair {}", pair_id);
            } else {
                log::error!("Pair {} not found in examples.pairs", pair_id);
            }
        }
        Err(err) => {
            log::error!("Failed to fetch example data: {}", err);
            // TODO: Show error to user
        }
    }
    Command::None
}

pub fn handle_cycle_example_pair(state: &mut State) -> Command<Msg> {
    use crate::tui::Resource;
    use super::super::matching_adapter::recompute_all_matches;

    // Cycle through pairs, or toggle off if at end
    if state.examples.pairs.is_empty() {
        // No pairs, just toggle
        state.examples.enabled = !state.examples.enabled;
        state.examples.active_pair_id = None;
    } else if !state.examples.enabled {
        // Not enabled, enable and select first
        state.examples.enabled = true;
        state.examples.active_pair_id = state.examples.pairs.first().map(|p| p.id.clone());
    } else if let Some(active_id) = &state.examples.active_pair_id {
        // Find current pair index
        let current_idx = state.examples.pairs.iter()
            .position(|p| &p.id == active_id);

        if let Some(idx) = current_idx {
            // Move to next, or toggle off if at end
            let next_idx = idx + 1;
            if next_idx >= state.examples.pairs.len() {
                // At end, toggle off
                state.examples.enabled = false;
                state.examples.active_pair_id = None;
            } else {
                // Move to next
                state.examples.active_pair_id = Some(state.examples.pairs[next_idx].id.clone());
            }
        } else {
            // Active ID not found, select first
            state.examples.active_pair_id = state.examples.pairs.first().map(|p| p.id.clone());
        }
    } else {
        // Enabled but no active pair, select first
        state.examples.active_pair_id = state.examples.pairs.first().map(|p| p.id.clone());
    }

    // Recompute matches since the active example pair changed
    if let (Resource::Success(source), Resource::Success(target)) =
        (&state.source_metadata, &state.target_metadata)
    {
        let (field_matches, relationship_matches, entity_matches, source_entities, target_entities) =
            recompute_all_matches(
                source,
                target,
                &state.field_mappings,
                &state.imported_mappings,
                &state.prefix_mappings,
                &state.examples,
                &state.source_entity,
                &state.target_entity,
                &state.negative_matches,
            );
        state.field_matches = field_matches;
        state.relationship_matches = relationship_matches;
        state.entity_matches = entity_matches;
        state.source_entities = source_entities;
        state.target_entities = target_entities;
    }

    Command::None
}

pub fn handle_toggle_examples(state: &mut State) -> Command<Msg> {
    state.examples.toggle();
    Command::None
}
