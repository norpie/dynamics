//! Transfer Preview app - displays resolved records after transform

use crate::tui::resource::Resource;
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use super::state::{Msg, PreviewParams, State};
use super::view;

/// Transfer Preview App - shows resolved records before execution
pub struct TransferPreviewApp;

impl crate::tui::AppState for State {}

impl App for TransferPreviewApp {
    type State = State;
    type Msg = Msg;
    type InitParams = PreviewParams;

    fn init(params: PreviewParams) -> (State, Command<Msg>) {
        let state = State {
            config_name: params.config_name.clone(),
            source_env: params.source_env.clone(),
            target_env: params.target_env.clone(),
            resolved: Resource::Loading,
            ..Default::default()
        };

        // TODO (Chunk 3): Load config and run transform engine
        // For now, just return None command - data loading will be implemented
        // in Chunk 3 when we wire up the transform engine
        let cmd = Command::None;

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            // Data loading
            Msg::ResolvedLoaded(result) => {
                state.resolved = match result {
                    Ok(resolved) => Resource::Success(resolved),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            // Navigation within table
            Msg::ListEvent(event) => {
                // TODO (Chunk 4): Pass real item count and visible height
                let item_count = if let Resource::Success(resolved) = &state.resolved {
                    resolved.entities.get(state.current_entity_idx)
                        .map(|e| e.records.len())
                        .unwrap_or(0)
                } else {
                    0
                };
                let visible_height = 20; // Placeholder until table is implemented
                state.list_state.handle_event(event, item_count, visible_height);
                Command::None
            }

            Msg::NextEntity => {
                if let Resource::Success(resolved) = &state.resolved {
                    if state.current_entity_idx + 1 < resolved.entities.len() {
                        state.current_entity_idx += 1;
                        state.list_state = crate::tui::widgets::ListState::with_selection();
                    }
                }
                Command::None
            }

            Msg::PrevEntity => {
                if state.current_entity_idx > 0 {
                    state.current_entity_idx -= 1;
                    state.list_state = crate::tui::widgets::ListState::with_selection();
                }
                Command::None
            }

            Msg::SelectEntity(idx) => {
                if let Resource::Success(resolved) = &state.resolved {
                    if idx < resolved.entities.len() {
                        state.current_entity_idx = idx;
                        state.list_state = crate::tui::widgets::ListState::with_selection();
                    }
                }
                Command::None
            }

            // Filtering
            Msg::SetFilter(filter) => {
                state.filter = filter;
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            Msg::CycleFilter => {
                state.filter = state.filter.next();
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            Msg::SearchChanged(event) => {
                // TODO: Implement search handling
                let _ = event;
                Command::None
            }

            // Record actions
            Msg::ToggleSkip => {
                // TODO (Chunk 8): Implement skip toggle
                Command::None
            }

            Msg::ViewDetails => {
                if let Some(idx) = state.list_state.selected() {
                    state.active_modal = Some(super::state::PreviewModal::RecordDetails {
                        record_idx: idx,
                    });
                }
                Command::None
            }

            Msg::EditRecord => {
                if let Some(idx) = state.list_state.selected() {
                    state.active_modal = Some(super::state::PreviewModal::EditRecord {
                        record_idx: idx,
                    });
                }
                Command::None
            }

            Msg::SaveRecord => {
                // TODO (Chunk 7): Implement record save
                state.active_modal = None;
                Command::None
            }

            // Bulk actions
            Msg::OpenBulkActions => {
                state.active_modal = Some(super::state::PreviewModal::BulkActions);
                Command::None
            }

            Msg::ApplyBulkAction(_action) => {
                // TODO (Chunk 8): Implement bulk action
                state.active_modal = None;
                Command::None
            }

            // Excel
            Msg::ExportExcel => {
                // TODO (Chunk 9): Implement Excel export
                Command::None
            }

            Msg::ImportExcel => {
                // TODO (Chunk 10): Implement Excel import
                Command::None
            }

            Msg::ExportCompleted(result) => {
                match result {
                    Ok(path) => log::info!("Exported to {}", path),
                    Err(e) => log::error!("Export failed: {}", e),
                }
                Command::None
            }

            Msg::ImportCompleted(result) => {
                match result {
                    Ok(resolved) => {
                        state.resolved = Resource::Success(resolved);
                    }
                    Err(e) => log::error!("Import failed: {}", e),
                }
                state.active_modal = None;
                Command::None
            }

            // Refresh
            Msg::Refresh => {
                // TODO (Chunk 11): Re-run transform
                Command::None
            }

            // Modal
            Msg::CloseModal => {
                state.active_modal = None;
                Command::None
            }

            // Navigation
            Msg::Back => {
                Command::navigate_to(AppId::TransferMappingEditor)
            }

            Msg::GoToExecute => {
                // TODO (Chunk 12): Navigate to execute app
                log::info!("Would navigate to execute");
                Command::None
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        view::render(state, theme)
    }

    fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
        view::subscriptions(state)
    }

    fn title() -> &'static str {
        "Transfer Preview"
    }
}
