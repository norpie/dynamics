//! Operation Queue App

use crate::tui::{
    app::App,
    command::{AppId, Command},
    element::{Element, LayoutConstraint, FocusId, Alignment},
    subscription::Subscription,
    state::theme::Theme,
    renderer::LayeredView,
    widgets::{TreeState, TreeEvent, ScrollableState},
    ModalState,
};
use crate::{col, row, use_constraints};
use crate::api::resilience::ResilienceConfig;
use ratatui::text::Line;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use super::models::{QueueItem, QueueFilter, SortMode, OperationStatus, QueueResult};
use super::tree_nodes::QueueTreeNode;
use super::commands::{save_settings_command, execute_next_if_available, execute_up_to_max};
use super::utils::estimate_remaining_time;
use super::views::{build_details_panel, build_clear_confirm_modal, build_delete_confirm_modal, build_interruption_warning_modal};

pub struct OperationQueueApp;

#[derive(Clone)]
pub enum Msg {
    // Tree interaction
    TreeEvent(TreeEvent),
    NodeSelected(String),
    ViewportHeight(usize),

    // Queue controls
    TogglePlay,
    StepOne,
    IncreasePriority(String),
    DecreasePriority(String),
    TogglePauseItem(String),
    DeleteItem(String),
    RetryItem(String),

    // Keyboard shortcuts for selected item
    IncreasePrioritySelected,
    DecreasePrioritySelected,
    TogglePauseSelected,
    DeleteSelected,
    RetrySelected,

    // Queue management
    AddItems(Vec<QueueItem>),
    RequestClearQueue,
    ConfirmClearQueue,
    RequestDeleteSelected,
    ConfirmDeleteSelected,
    CancelModal,

    // Execution
    StartExecution(String),
    ExecutionCompleted(String, QueueResult),

    // Filters/Settings
    SetFilter(QueueFilter),
    SetSortMode(SortMode),
    SetMaxConcurrent(usize),

    // Details panel scrolling
    DetailsScroll(crossterm::event::KeyCode),
    DetailsSetDimensions(usize, usize, usize, usize),  // (viewport_height, content_height, viewport_width, content_width)

    // State loading and persistence
    StateLoaded(Result<(Vec<QueueItem>, crate::config::repository::queue::QueueSettings, Vec<QueueItem>), String>),
    PersistenceError(String),

    // Interruption warnings
    DismissInterruptionWarning,
    ClearInterruptionFlag(String),
    ClearInterruptionFlagSelected,

    // Navigation
    Back,
}

pub struct State {
    // Queue data (Arc for cheap cloning when building tree nodes)
    pub queue_items: Vec<Arc<QueueItem>>,
    pub tree_state: TreeState,

    // Cached sorted indices (rebuilt only when queue/filter/sort changes)
    cached_sorted_indices: Vec<usize>,
    index_cache_valid: bool,

    // Execution state
    pub auto_play: bool,
    pub max_concurrent: usize,
    pub currently_running: HashSet<String>,

    // Performance tracking
    pub recent_completion_times: VecDeque<u64>, // Store last 10 completion times in ms

    // UI state
    pub filter: QueueFilter,
    pub sort_mode: SortMode,
    pub selected_item_id: Option<String>,
    pub details_scroll_state: ScrollableState,

    // Modals
    pub clear_confirm_modal: ModalState<()>,
    pub delete_confirm_modal: ModalState<()>,
    pub interruption_warning_modal: ModalState<Vec<QueueItem>>,

    // Loading state
    pub is_loading: bool,
}

impl Default for State {
    fn default() -> Self {
        // Use centralized resilience config default for max_concurrent
        let default_max_concurrent = ResilienceConfig::default().concurrency.max_queue_items;
        Self {
            queue_items: Vec::new(),
            tree_state: TreeState::with_selection(),
            cached_sorted_indices: Vec::new(),
            index_cache_valid: false,
            auto_play: false,
            max_concurrent: default_max_concurrent,
            currently_running: HashSet::new(),
            recent_completion_times: VecDeque::with_capacity(10),
            filter: QueueFilter::All,
            sort_mode: SortMode::Priority,
            selected_item_id: None,
            details_scroll_state: ScrollableState::new(),
            clear_confirm_modal: ModalState::Closed,
            delete_confirm_modal: ModalState::Closed,
            interruption_warning_modal: ModalState::Closed,
            is_loading: true,
        }
    }
}

impl State {
    /// Invalidate the index cache (call when queue_items/filter/sort change)
    pub fn invalidate_index_cache(&mut self) {
        self.index_cache_valid = false;
    }

    /// Rebuild index cache if invalid
    fn rebuild_index_cache_if_needed(&mut self) {
        if self.index_cache_valid {
            return;
        }

        let mut filtered_indices: Vec<usize> = self
            .queue_items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.filter.matches(item))
            .map(|(idx, _)| idx)
            .collect();

        filtered_indices.sort_by(|&a, &b| {
            let item_a = &self.queue_items[a];
            let item_b = &self.queue_items[b];
            match self.sort_mode {
                SortMode::Priority => item_a.priority.cmp(&item_b.priority),
                SortMode::Status => {
                    format!("{:?}", item_a.status).cmp(&format!("{:?}", item_b.status))
                }
                SortMode::Source => item_a.metadata.source.cmp(&item_b.metadata.source),
            }
        });

        self.cached_sorted_indices = filtered_indices;
        self.index_cache_valid = true;
    }

    /// Build tree nodes from cached indices (called each frame - Arc::clone is O(1))
    fn build_tree_nodes(&self) -> Vec<QueueTreeNode> {
        self.cached_sorted_indices
            .iter()
            .filter_map(|&idx| self.queue_items.get(idx))
            .map(|item| QueueTreeNode::from_arc(Arc::clone(item)))
            .collect()
    }

    /// Mutate a queue item by id. Clones the inner data, applies mutation, replaces Arc.
    /// Returns true if item was found and mutated.
    pub fn mutate_item<F>(&mut self, id: &str, f: F) -> bool
    where
        F: FnOnce(&mut QueueItem),
    {
        if let Some(idx) = self.queue_items.iter().position(|item| item.id == id) {
            let mut item = (*self.queue_items[idx]).clone();
            f(&mut item);
            self.queue_items[idx] = Arc::new(item);
            true
        } else {
            false
        }
    }
}

impl crate::tui::AppState for State {}

impl App for OperationQueueApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: ()) -> (State, Command<Msg>) {
        let cmd = Command::perform(
            async move {
                let config = crate::global_config();

                // Load queue items
                let mut queue_items = config.list_queue_items().await
                    .map_err(|e| format!("Failed to load queue items: {}", e))?;

                // Load settings (filter/sort from queue settings, max_concurrent from resilience)
                let mut settings = config.get_queue_settings().await
                    .map_err(|e| format!("Failed to load queue settings: {}", e))?;

                // Override max_concurrent with centralized resilience config
                let resilience = ResilienceConfig::load_from_options().await
                    .unwrap_or_default();
                settings.max_concurrent = resilience.concurrency.max_queue_items;

                // Detect and handle interrupted items
                let mut interrupted_items = Vec::new();
                let now = chrono::Utc::now();

                log::info!("Checking for interrupted items. Total items loaded: {}", queue_items.len());

                for item in &mut queue_items {
                    log::debug!("Item {} has status: {:?}", item.id, item.status);
                    if item.status == OperationStatus::Running {
                        log::warn!("Found interrupted item: {} (was Running)", item.id);

                        // Mark as interrupted
                        item.status = OperationStatus::Pending;
                        item.was_interrupted = true;
                        item.interrupted_at = Some(now);
                        item.started_at = None;

                        interrupted_items.push(item.clone());

                        // Persist the changes
                        config.update_queue_item_status(&item.id, OperationStatus::Pending).await
                            .map_err(|e| format!("Failed to update status: {}", e))?;
                        config.mark_queue_item_interrupted(&item.id, now).await
                            .map_err(|e| format!("Failed to mark interrupted: {}", e))?;
                    }
                }

                log::info!("Found {} interrupted items", interrupted_items.len());

                Ok((queue_items, settings, interrupted_items))
            },
            Msg::StateLoaded
        );

        (State::default(), cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::StateLoaded(result) => {
                match result {
                    Ok((queue_items, settings, interrupted_items)) => {
                        state.queue_items = queue_items.into_iter().map(Arc::new).collect();
                        state.invalidate_index_cache();
                        state.auto_play = false; // Always start paused on load
                        state.max_concurrent = settings.max_concurrent;
                        state.filter = settings.filter;
                        state.sort_mode = settings.sort_mode;
                        state.is_loading = false;

                        // Auto-select first item if queue is not empty
                        if !state.queue_items.is_empty() && state.selected_item_id.is_none() {
                            state.selected_item_id = state.queue_items.first().map(|item| item.id.clone());
                        }

                        // Show warning modal if there are interrupted items
                        if !interrupted_items.is_empty() {
                            state.interruption_warning_modal.open_with(interrupted_items);
                            Command::set_focus(FocusId::new("warning-close"))
                        } else {
                            Command::set_focus(FocusId::new("queue-tree"))
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to load queue state: {}", err);
                        state.is_loading = false;
                        Command::None
                    }
                }
            }

            Msg::PersistenceError(err) => {
                log::error!("Queue persistence error: {}", err);
                Command::None
            }

            Msg::DismissInterruptionWarning => {
                state.interruption_warning_modal.close();
                Command::set_focus(FocusId::new("queue-tree"))
            }

            Msg::ClearInterruptionFlag(id) => {
                if state.mutate_item(&id, |item| {
                    item.was_interrupted = false;
                    item.interrupted_at = None;
                }) {
                    state.invalidate_index_cache();

                    // Persist to database
                    let item_id = id.clone();
                    return Command::perform(
                        async move {
                            let config = crate::global_config();
                            config.clear_queue_interruption_flag(&item_id).await
                                .map_err(|e| format!("Failed to clear interruption flag: {}", e))
                        },
                        |result| {
                            match result {
                                Err(err) => Msg::PersistenceError(err),
                                Ok(_) => Msg::PersistenceError("".to_string()),
                            }
                        }
                    );
                }
                Command::None
            }

            Msg::ClearInterruptionFlagSelected => {
                if let Some(id) = &state.selected_item_id {
                    return Self::update(state, Msg::ClearInterruptionFlag(id.clone()));
                }
                Command::None
            }

            Msg::TreeEvent(event) => {
                let old_selected = state.selected_item_id.clone();
                state.tree_state.handle_event(event);
                // Update selected item when navigating (not just on Enter)
                let new_selected = state.tree_state.selected().map(|s| s.to_string());

                // Reset scroll state when selection changes
                if old_selected != new_selected {
                    state.details_scroll_state = ScrollableState::new();
                }

                state.selected_item_id = new_selected;
                Command::None
            }

            Msg::NodeSelected(id) => {
                // Reset scroll state when selecting a new item
                if state.selected_item_id.as_ref() != Some(&id) {
                    state.details_scroll_state = ScrollableState::new();
                }
                state.selected_item_id = Some(id);
                Command::None
            }

            Msg::ViewportHeight(height) => {
                state.tree_state.set_viewport_height(height);
                state.tree_state.update_scroll(height);
                Command::None
            }

            Msg::TogglePlay => {
                state.auto_play = !state.auto_play;

                let save_cmd = save_settings_command(state);
                let exec_cmd = if state.auto_play {
                    execute_up_to_max(state)
                } else {
                    Command::None
                };

                Command::Batch(vec![save_cmd, exec_cmd])
            }

            Msg::StepOne => {
                state.auto_play = false;

                // Check if selected item is Failed or PartiallyFailed - if so, retry it directly
                if let Some(ref selected_id) = state.selected_item_id {
                    let selected_status = state.queue_items.iter()
                        .find(|i| &i.id == selected_id)
                        .map(|i| i.status.clone());

                    if matches!(selected_status, Some(OperationStatus::Failed) | Some(OperationStatus::PartiallyFailed)) {
                        let id = selected_id.clone();

                        // Reset item for retry
                        state.mutate_item(&id, |item| {
                            item.status = OperationStatus::Pending;
                            item.result = None;
                            item.started_at = None;
                        });
                        state.invalidate_index_cache();

                        // Persist Pending status and then trigger StartExecution
                        let item_id = id.clone();
                        let persist_cmd = Command::perform(
                            async move {
                                crate::global_config().update_queue_item_status(&item_id, OperationStatus::Pending).await
                                    .map_err(|e| format!("Failed to update status: {}", e))
                            },
                            |result| {
                                if let Err(err) = result {
                                    Msg::PersistenceError(err)
                                } else {
                                    Msg::PersistenceError("".to_string())
                                }
                            }
                        );

                        // Execute this item directly
                        let exec_cmd = Command::perform(async move { id }, Msg::StartExecution);

                        return Command::Batch(vec![persist_cmd, exec_cmd]);
                    }
                }

                // Default: execute next available pending item
                execute_next_if_available(state)
            }

            Msg::IncreasePriority(id) => {
                // First read current priority
                let current = state.queue_items.iter().find(|i| i.id == id).map(|i| i.priority);
                let new_priority = if let Some(p) = current {
                    if p > 0 {
                        state.mutate_item(&id, |item| item.priority -= 1);
                        Some(p - 1)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(priority) = new_priority {
                    state.invalidate_index_cache();
                    let item_id = id.clone();
                    return Command::perform(
                        async move {
                            crate::global_config().update_queue_item_priority(&item_id, priority).await
                                .map_err(|e| format!("Failed to update priority: {}", e))
                        },
                        |result| {
                            if let Err(err) = result {
                                Msg::PersistenceError(err)
                            } else {
                                Msg::PersistenceError("".to_string())
                            }
                        }
                    );
                }
                Command::None
            }

            Msg::DecreasePriority(id) => {
                // First read current priority
                let current = state.queue_items.iter().find(|i| i.id == id).map(|i| i.priority);
                let new_priority = if let Some(p) = current {
                    if p < 255 {
                        state.mutate_item(&id, |item| item.priority += 1);
                        Some(p + 1)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(priority) = new_priority {
                    state.invalidate_index_cache();
                    let item_id = id.clone();
                    return Command::perform(
                        async move {
                            crate::global_config().update_queue_item_priority(&item_id, priority).await
                                .map_err(|e| format!("Failed to update priority: {}", e))
                        },
                        |result| {
                            if let Err(err) = result {
                                Msg::PersistenceError(err)
                            } else {
                                Msg::PersistenceError("".to_string())
                            }
                        }
                    );
                }
                Command::None
            }

            Msg::TogglePauseItem(id) => {
                // Read current status
                let current_status = state.queue_items.iter().find(|i| i.id == id).map(|i| i.status.clone());
                if let Some(status) = current_status {
                    let new_status = match status {
                        OperationStatus::Pending => OperationStatus::Paused,
                        OperationStatus::Paused => OperationStatus::Pending,
                        _ => status,
                    };
                    state.mutate_item(&id, |item| item.status = new_status.clone());
                    state.invalidate_index_cache();

                    let item_id = id.clone();
                    return Command::perform(
                        async move {
                            crate::global_config().update_queue_item_status(&item_id, new_status).await
                                .map_err(|e| format!("Failed to update status: {}", e))
                        },
                        |result| {
                            if let Err(err) = result {
                                Msg::PersistenceError(err)
                            } else {
                                Msg::PersistenceError("".to_string())
                            }
                        }
                    );
                }
                Command::None
            }

            Msg::DeleteItem(id) => {
                state.queue_items.retain(|item| item.id != id);
                state.invalidate_index_cache();
                state.tree_state.invalidate_cache();

                Command::perform(
                    async move {
                        crate::global_config().delete_queue_item(&id).await
                            .map_err(|e| format!("Failed to delete queue item: {}", e))
                    },
                    |result| {
                        if let Err(err) = result {
                            Msg::PersistenceError(err)
                        } else {
                            Msg::PersistenceError("".to_string())
                        }
                    }
                )
            }

            Msg::RetryItem(id) => {
                if state.mutate_item(&id, |item| {
                    item.status = OperationStatus::Pending;
                    item.result = None;
                    item.started_at = None;
                }) {
                    state.invalidate_index_cache();

                    let item_id = id.clone();
                    let persist_cmd = Command::perform(
                        async move {
                            crate::global_config().update_queue_item_status(&item_id, OperationStatus::Pending).await
                                .map_err(|e| format!("Failed to update status: {}", e))
                        },
                        |result| {
                            if let Err(err) = result {
                                Msg::PersistenceError(err)
                            } else {
                                Msg::PersistenceError("".to_string())
                            }
                        }
                    );

                    let exec_cmd = if state.auto_play {
                        execute_next_if_available(state)
                    } else {
                        Command::None
                    };

                    return Command::Batch(vec![persist_cmd, exec_cmd]);
                }
                Command::None
            }

            Msg::StartExecution(id) => {
                // Mark as running and set start time
                state.mutate_item(&id, |item| {
                    item.status = OperationStatus::Running;
                    item.started_at = Some(std::time::Instant::now());
                });
                state.currently_running.insert(id.clone());
                state.invalidate_index_cache();

                // Persist Running status to database
                let item_id_for_persist = id.clone();
                let persist_cmd = Command::perform(
                    async move {
                        log::info!("Persisting Running status for item: {}", item_id_for_persist);
                        let result = crate::global_config().update_queue_item_status(&item_id_for_persist, OperationStatus::Running).await;
                        match &result {
                            Ok(_) => log::info!("Successfully persisted Running status for item: {}", item_id_for_persist),
                            Err(e) => log::error!("Failed to persist Running status: {}", e),
                        }
                        result.map_err(|e| format!("Failed to persist Running status: {}", e))
                    },
                    |result| {
                        if let Err(err) = result {
                            Msg::PersistenceError(err)
                        } else {
                            Msg::PersistenceError("".to_string())
                        }
                    }
                );

                // Get item for execution
                let item = state.queue_items.iter().find(|i| i.id == id).cloned();

                if let Some(item) = item {
                    // Get operations to execute (excluding already-succeeded ones)
                    let ops_to_execute = if item.succeeded_indices.is_empty() {
                        item.operations.clone()
                    } else {
                        log::info!(
                            "Retrying item {} with {} succeeded indices, executing {} of {} operations",
                            item.id,
                            item.succeeded_indices.len(),
                            item.operations.len() - item.succeeded_indices.len(),
                            item.operations.len()
                        );
                        item.operations.without_indices(&item.succeeded_indices)
                    };

                    let exec_cmd = Command::perform(
                        async move {
                            let start = std::time::Instant::now();

                            // Get client for this environment from global client manager
                            let client = match crate::client_manager().get_client(&item.metadata.environment_name).await {
                                Ok(client) => client,
                                Err(e) => {
                                    let duration_ms = start.elapsed().as_millis() as u64;
                                    return (item.id.clone(), QueueResult {
                                        success: false,
                                        operation_results: vec![],
                                        error: Some(format!("Failed to get client: {}", e)),
                                        duration_ms,
                                    });
                                }
                            };

                            let resilience = ResilienceConfig::default();
                            let result = ops_to_execute.execute(&client, &resilience).await;
                            let duration_ms = start.elapsed().as_millis() as u64;

                            let queue_result = match result {
                                Ok(operation_results) => QueueResult {
                                    success: operation_results.iter().all(|r| r.success),
                                    operation_results,
                                    error: None,
                                    duration_ms,
                                },
                                Err(e) => QueueResult {
                                    success: false,
                                    operation_results: vec![],
                                    error: Some(e.to_string()),
                                    duration_ms,
                                },
                            };

                            (item.id.clone(), queue_result)
                        },
                        |(id, result)| Msg::ExecutionCompleted(id, result),
                    );

                    Command::Batch(vec![persist_cmd, exec_cmd])
                } else {
                    persist_cmd
                }
            }

            Msg::ExecutionCompleted(id, result) => {
                state.currently_running.remove(&id);
                state.invalidate_index_cache();

                let mut publish_cmd = Command::None;
                let mut persist_cmd = Command::None;

                // First, read the item to compute new state
                let item_data = state.queue_items.iter().find(|i| i.id == id).map(|item| {
                    let succeeded_count = result.operation_results.iter().filter(|r| r.success).count();
                    let failed_count = result.operation_results.iter().filter(|r| !r.success).count();
                    let total_count = result.operation_results.len();

                    let (new_status, new_succeeded_indices) = if result.success {
                        (OperationStatus::Done, vec![])
                    } else if succeeded_count > 0 && failed_count > 0 {
                        let newly_succeeded: Vec<usize> = result.operation_results
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| r.success)
                            .map(|(idx, _)| {
                                if item.succeeded_indices.is_empty() {
                                    idx
                                } else {
                                    let mut original_idx = 0;
                                    let mut count = 0;
                                    for i in 0..item.operations.len() {
                                        if !item.succeeded_indices.contains(&i) {
                                            if count == idx {
                                                original_idx = i;
                                                break;
                                            }
                                            count += 1;
                                        }
                                    }
                                    original_idx
                                }
                            })
                            .collect();

                        let mut merged = item.succeeded_indices.clone();
                        for idx in newly_succeeded {
                            if !merged.contains(&idx) {
                                merged.push(idx);
                            }
                        }
                        merged.sort();
                        (OperationStatus::PartiallyFailed, merged)
                    } else if total_count == 0 && result.error.is_some() {
                        (OperationStatus::Failed, item.succeeded_indices.clone())
                    } else {
                        (OperationStatus::Failed, item.succeeded_indices.clone())
                    };

                    (new_status, new_succeeded_indices, item.metadata.clone())
                });

                if let Some((new_status, new_succeeded_indices, metadata)) = item_data {
                    // Now mutate the item
                    let new_status_clone = new_status.clone();
                    let new_succeeded_indices_clone = new_succeeded_indices.clone();
                    let result_clone = result.clone();
                    state.mutate_item(&id, move |item| {
                        item.status = new_status_clone;
                        item.result = Some(result_clone);
                        item.succeeded_indices = new_succeeded_indices_clone;
                    });

                    // Persist to database
                    let item_id = id.clone();
                    let result_for_persist = result.clone();
                    persist_cmd = Command::perform(
                        async move {
                            let config = crate::global_config();
                            config.update_queue_item_status(&item_id, new_status).await
                                .map_err(|e| format!("Failed to update status: {}", e))?;
                            config.update_queue_item_result(&item_id, &result_for_persist).await
                                .map_err(|e| format!("Failed to update result: {}", e))?;
                            config.update_queue_item_succeeded_indices(&item_id, &new_succeeded_indices).await
                                .map_err(|e| format!("Failed to update succeeded_indices: {}", e))?;
                            Ok(())
                        },
                        |result| {
                            if let Err(err) = result {
                                Msg::PersistenceError(err)
                            } else {
                                Msg::PersistenceError("".to_string())
                            }
                        }
                    );

                    // Track completion time for successful operations
                    if result.success {
                        state.recent_completion_times.push_back(result.duration_ms);
                        if state.recent_completion_times.len() > 10 {
                            state.recent_completion_times.pop_front();
                        }
                    }

                    // Log completion with error details
                    if result.success {
                        log::info!(
                            "✓ Queue item {} completed successfully: duration={}ms",
                            id,
                            result.duration_ms
                        );
                    } else {
                        log::error!(
                            "✗ Queue item {} FAILED: duration={}ms",
                            id,
                            result.duration_ms
                        );

                        if let Some(ref error) = result.error {
                            log::error!("  Error: {}", error);
                        }

                        for (idx, op_result) in result.operation_results.iter().enumerate() {
                            if !op_result.success {
                                log::error!(
                                    "  Operation {} failed: {} on entity '{}' (status: {})",
                                    idx + 1,
                                    op_result.operation.operation_type(),
                                    op_result.operation.entity(),
                                    op_result.status_code.map(|s| s.to_string()).unwrap_or_else(|| "unknown".to_string())
                                );
                                if let Some(ref err) = op_result.error {
                                    log::error!("    Details: {}", err);
                                }
                            }
                        }
                    }

                    // Publish completion event for subscribers
                    let completion_data = serde_json::json!({
                        "id": id,
                        "result": result,
                        "metadata": metadata,
                    });
                    publish_cmd = Command::Publish {
                        topic: "queue:item_completed".to_string(),
                        data: completion_data,
                    };
                }

                // Pause on failure - don't continue to next item if this one failed
                if !result.success && state.auto_play {
                    state.auto_play = false;
                    log::warn!("Queue paused due to failed operation");
                }

                // Continue if auto-play (will be false if we just paused due to failure)
                // Use execute_up_to_max to fill available slots at current priority tier
                let next_cmd = if state.auto_play {
                    execute_up_to_max(state)
                } else {
                    Command::None
                };

                Command::Batch(vec![publish_cmd, persist_cmd, next_cmd])
            }

            Msg::SetFilter(filter) => {
                state.filter = filter;
                state.invalidate_index_cache();
                state.tree_state.invalidate_cache();
                save_settings_command(state)
            }

            Msg::SetSortMode(sort_mode) => {
                state.sort_mode = sort_mode;
                state.invalidate_index_cache();
                state.tree_state.invalidate_cache();
                save_settings_command(state)
            }

            Msg::SetMaxConcurrent(max) => {
                state.max_concurrent = max;
                save_settings_command(state)
            }

            // Keyboard shortcuts operating on selected item
            Msg::IncreasePrioritySelected => {
                if let Some(id) = state.selected_item_id.clone() {
                    return Self::update(state, Msg::IncreasePriority(id));
                }
                Command::None
            }

            Msg::DecreasePrioritySelected => {
                if let Some(id) = state.selected_item_id.clone() {
                    return Self::update(state, Msg::DecreasePriority(id));
                }
                Command::None
            }

            Msg::TogglePauseSelected => {
                if let Some(id) = state.selected_item_id.clone() {
                    return Self::update(state, Msg::TogglePauseItem(id));
                }
                Command::None
            }

            Msg::RequestDeleteSelected => {
                // Only show modal if there's a selected item
                if state.selected_item_id.is_some() {
                    state.delete_confirm_modal.open_empty();
                    Command::set_focus(FocusId::new("confirmation-cancel"))
                } else {
                    Command::None
                }
            }

            Msg::ConfirmDeleteSelected => {
                state.delete_confirm_modal.close();
                if let Some(id) = &state.selected_item_id {
                    let item_id = id.clone();
                    state.queue_items.retain(|item| &item.id != id);
                    state.selected_item_id = None; // Clear selection after delete
                    state.invalidate_index_cache();
                    state.tree_state.select_and_scroll(None);
                    state.tree_state.invalidate_cache();

                    return Command::perform(
                        async move {
                            crate::global_config().delete_queue_item(&item_id).await
                                .map_err(|e| format!("Failed to delete queue item: {}", e))
                        },
                        |result| {
                            if let Err(err) = result {
                                Msg::PersistenceError(err)
                            } else {
                                Msg::PersistenceError("".to_string())
                            }
                        }
                    );
                }
                Command::None
            }

            Msg::DeleteSelected => {
                // Deprecated - use RequestDeleteSelected instead
                Command::None
            }

            Msg::RetrySelected => {
                let id = state.selected_item_id.clone();
                if let Some(id) = id {
                    let found = state.mutate_item(&id, |item| {
                        item.status = OperationStatus::Pending;
                        item.result = None;
                    });

                    if found {
                        state.invalidate_index_cache();

                        let item_id = id.clone();
                        let persist_cmd = Command::perform(
                            async move {
                                crate::global_config().update_queue_item_status(&item_id, OperationStatus::Pending).await
                                    .map_err(|e| format!("Failed to update status: {}", e))
                            },
                            |result| {
                                if let Err(err) = result {
                                    Msg::PersistenceError(err)
                                } else {
                                    Msg::PersistenceError("".to_string())
                                }
                            }
                        );

                        let exec_cmd = if state.auto_play {
                            execute_next_if_available(state)
                        } else {
                            Command::None
                        };

                        return Command::Batch(vec![persist_cmd, exec_cmd]);
                    }
                }
                Command::None
            }

            Msg::AddItems(items) => {
                let was_empty = state.queue_items.is_empty();

                // Persist each item to database
                let items_to_save = items.clone();
                let persist_cmd = Command::perform(
                    async move {
                        let config = crate::global_config();
                        for item in &items_to_save {
                            if let Err(e) = config.save_queue_item(item).await {
                                return Err(format!("Failed to save queue item: {}", e));
                            }
                        }
                        Ok(())
                    },
                    |result| {
                        if let Err(err) = result {
                            Msg::PersistenceError(err)
                        } else {
                            Msg::PersistenceError("".to_string())
                        }
                    }
                );

                let arc_items: Vec<Arc<QueueItem>> = items.into_iter().map(Arc::new).collect();
                state.queue_items.extend(arc_items);
                state.invalidate_index_cache();
                state.tree_state.invalidate_cache();

                // If queue was empty and we just added items, select the first one
                if was_empty && !state.queue_items.is_empty() && state.selected_item_id.is_none() {
                    state.selected_item_id = state.queue_items.first().map(|item| item.id.clone());
                }

                // If in play mode and we have capacity, start executing
                let exec_cmd = if state.auto_play && state.currently_running.len() < state.max_concurrent {
                    execute_next_if_available(state)
                } else {
                    Command::None
                };

                Command::Batch(vec![persist_cmd, exec_cmd])
            }

            Msg::RequestClearQueue => {
                state.clear_confirm_modal.open_empty();
                Command::set_focus(FocusId::new("confirmation-cancel"))
            }

            Msg::ConfirmClearQueue => {
                state.clear_confirm_modal.close();
                state.queue_items.clear();
                state.selected_item_id = None;
                state.invalidate_index_cache();
                state.tree_state.select_and_scroll(None);
                state.tree_state.invalidate_cache();

                Command::perform(
                    async move {
                        crate::global_config().clear_queue().await
                            .map_err(|e| format!("Failed to clear queue: {}", e))
                    },
                    |result| {
                        if let Err(err) = result {
                            Msg::PersistenceError(err)
                        } else {
                            Msg::PersistenceError("".to_string())
                        }
                    }
                )
            }

            Msg::CancelModal => {
                state.clear_confirm_modal.close();
                state.delete_confirm_modal.close();
                Command::None
            }

            Msg::DetailsScroll(key) => {
                // Dimensions are tracked from last on_render call
                let viewport_height = state.details_scroll_state.viewport_height().unwrap_or(20);
                let content_height = state.details_scroll_state.content_height().unwrap_or(20);
                state.details_scroll_state.handle_key(key, content_height, viewport_height);
                Command::None
            }

            Msg::DetailsSetDimensions(viewport_height, content_height, viewport_width, content_width) => {
                // Called every frame by renderer with actual dimensions
                state.details_scroll_state.set_viewport_height(viewport_height);
                state.details_scroll_state.update_scroll(viewport_height, content_height);
                state.details_scroll_state.set_viewport_width(viewport_width);
                state.details_scroll_state.update_horizontal_scroll(viewport_width, content_width);
                Command::None
            }

            Msg::Back => Command::navigate_to(AppId::AppLauncher),
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let view_start = std::time::Instant::now();
        use_constraints!();
        let theme = &crate::global_runtime_config().theme;

        // Rebuild index cache if needed (only when data/filter/sort changes)
        let cache_start = std::time::Instant::now();
        state.rebuild_index_cache_if_needed();
        // Build tree nodes fresh each frame (for live elapsed time updates)
        let tree_nodes = state.build_tree_nodes();
        let cache_elapsed = cache_start.elapsed();

        // Controls and stats row
        let play_button = if state.auto_play {
            Element::button("pause-btn", "[P] Pause").on_press(Msg::TogglePlay)
        } else {
            Element::button("play-btn", "[P] Play").on_press(Msg::TogglePlay)
        }
        .build();

        let step_button = Element::button("step-btn", "[s] Step")
            .on_press(Msg::StepOne)
            .build();

        let clear_button = Element::button("clear-btn", "[C] Clear")
            .on_press(Msg::RequestClearQueue)
            .build();

        let count_by_status = |status: OperationStatus| {
            state
                .queue_items
                .iter()
                .filter(|item| item.status == status)
                .count()
        };

        let stats_text = format!(
            "Total: {}  Pending: {}  Running: {}  Done: {}  Failed: {}",
            state.queue_items.len(),
            count_by_status(OperationStatus::Pending),
            state.currently_running.len(),
            count_by_status(OperationStatus::Done),
            count_by_status(OperationStatus::Failed),
        );

        // Time estimates
        let est_3 = estimate_remaining_time(state, 3).unwrap_or_else(|| "-".to_string());
        let est_5 = estimate_remaining_time(state, 5).unwrap_or_else(|| "-".to_string());
        let est_10 = estimate_remaining_time(state, 10).unwrap_or_else(|| "-".to_string());

        let estimates_text = format!(
            "⏱ Est. remaining (last 3/5/10): {} / {} / {}",
            est_3, est_5, est_10
        );

        let buttons = row![
            play_button => Length(14),
            Element::None => Length(1),
            step_button => Length(11),
            Element::None => Length(1),
            clear_button => Length(11),
        ];

        let stats_and_estimates = col![
            Element::text(stats_text) => Length(1),
            Element::None => Length(1),
            Element::text(estimates_text) => Length(1),
        ];

        let header = row![
            buttons => Length(38),
            Element::None => Length(2),
            stats_and_estimates => Fill(1),
        ];

        // Table tree
        let element_build_start = std::time::Instant::now();
        let tree_widget = Element::table_tree("queue-tree", &tree_nodes, &mut state.tree_state)
            .on_event(Msg::TreeEvent)
            .on_select(Msg::NodeSelected)
            .on_render(Msg::ViewportHeight)
            .build();
        let element_build_elapsed = element_build_start.elapsed();

        let tree = Element::panel(tree_widget)
            .title("Queue")
            .build();

        // Build details panel for selected item
        let details_panel = build_details_panel(state, &state.details_scroll_state);

        // Split into tree (left) and details (right) - 2/1 ratio
        let main_content = row![
            col![
                header => Length(3),
                tree => Fill(1),
            ] => Fill(2),
            details_panel => Fill(1),
        ];

        let mut view = LayeredView::new(Element::panel(main_content).build());

        // Add clear confirmation modal if open
        if state.clear_confirm_modal.is_open() {
            let modal = build_clear_confirm_modal();
            view = view.with_app_modal(modal, Alignment::Center);
        }

        // Add delete confirmation modal if open
        if state.delete_confirm_modal.is_open() {
            let modal = build_delete_confirm_modal();
            view = view.with_app_modal(modal, Alignment::Center);
        }

        // Add interruption warning modal if open
        if state.interruption_warning_modal.is_open() {
            let modal = build_interruption_warning_modal(state);
            view = view.with_app_modal(modal, Alignment::Center);
        }

        let total_elapsed = view_start.elapsed();
        if total_elapsed.as_millis() > 5 {
            log::warn!(
                "PERF view(): total={}ms cache={}us element_build={}us items={}",
                total_elapsed.as_millis(),
                cache_elapsed.as_micros(),
                element_build_elapsed.as_micros(),
                state.queue_items.len()
            );
        }

        view
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        use crate::tui::{Subscription, KeyBinding};
        use crossterm::event::KeyCode;

        vec![
            // Keyboard shortcuts
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('P')), "Toggle play/pause (queue)", Msg::TogglePlay),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('p')), "Toggle pause (selected)", Msg::TogglePauseSelected),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('s')), "Step one operation", Msg::StepOne),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('C')), "Clear queue", Msg::RequestClearQueue),
            Subscription::keyboard(KeyBinding::new(KeyCode::Esc), "Back to launcher", Msg::Back),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('=')), "Increase priority (selected)", Msg::IncreasePrioritySelected),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('+')), "Increase priority (selected)", Msg::IncreasePrioritySelected),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('-')), "Decrease priority (selected)", Msg::DecreasePrioritySelected),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('r')), "Retry (selected)", Msg::RetrySelected),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('d')), "Delete (selected)", Msg::RequestDeleteSelected),
            Subscription::keyboard(KeyBinding::new(KeyCode::Char('c')), "Clear interruption warning (selected)", Msg::ClearInterruptionFlagSelected),

            // Event subscriptions
            Subscription::subscribe("queue:add_items", |value| {
                // Deserialize Vec<QueueItem> from JSON
                serde_json::from_value::<Vec<QueueItem>>(value)
                    .ok()
                    .map(Msg::AddItems)
            }),
        ]
    }

    fn title() -> &'static str {
        "Operation Queue"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        use ratatui::text::Span;
        let theme = &crate::global_runtime_config().theme;
        use ratatui::style::Style;

        let interrupted_count = state.queue_items.iter()
            .filter(|item| item.was_interrupted)
            .count();

        if interrupted_count > 0 {
            Some(Line::from(vec![
                Span::styled("⚠ ", Style::default().fg(theme.accent_error)),
                Span::styled(
                    format!("{} interrupted operation(s) - verify before resuming", interrupted_count),
                    Style::default().fg(theme.accent_warning)
                ),
            ]))
        } else {
            None
        }
    }

    fn suspend_policy() -> crate::tui::SuspendPolicy {
        crate::tui::SuspendPolicy::AlwaysActive
    }
}
