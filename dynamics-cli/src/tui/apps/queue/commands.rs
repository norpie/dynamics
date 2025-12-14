//! Command helpers for queue execution

use crate::tui::command::Command;
use super::app::{State, Msg};
use super::models::{OperationStatus, QueueResult};

/// Helper function to save queue settings to database
/// Note: auto_play is NOT persisted (always starts paused)
pub fn save_settings_command(state: &State) -> Command<Msg> {
    let settings = crate::config::repository::queue::QueueSettings {
        auto_play: false, // Never persist auto_play - always start paused
        max_concurrent: state.max_concurrent,
        filter: state.filter,
        sort_mode: state.sort_mode,
    };

    Command::perform(
        async move {
            crate::global_config().save_queue_settings(&settings).await
                .map_err(|e| format!("Failed to save queue settings: {}", e))
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

/// Find the current priority tier (minimum priority among Pending or Running items).
/// We must complete all items at a priority level before starting items at higher priorities
/// because higher priority numbers may have dependencies on lower ones.
fn current_priority_tier(state: &State) -> Option<u8> {
    state.queue_items.iter()
        .filter(|i| i.status == OperationStatus::Pending || i.status == OperationStatus::Running)
        .map(|i| i.priority)
        .min()
}

/// Execute multiple items at the current priority tier, up to max_concurrent.
/// This is the main entry point for starting queue execution.
pub fn execute_up_to_max(state: &mut State) -> Command<Msg> {
    let mut commands = Vec::new();

    while state.currently_running.len() < state.max_concurrent {
        match execute_next_if_available(state) {
            Command::None => break,
            cmd => commands.push(cmd),
        }
    }

    if commands.is_empty() {
        Command::None
    } else if commands.len() == 1 {
        commands.pop().unwrap()
    } else {
        Command::Batch(commands)
    }
}

/// Helper function to execute the next available operation at the current priority tier.
/// Only starts items at the minimum priority level among Pending+Running items,
/// ensuring dependency ordering is respected.
pub fn execute_next_if_available(state: &mut State) -> Command<Msg> {
    // Check if we can run more
    if state.currently_running.len() >= state.max_concurrent {
        return Command::None;
    }

    // Find the current priority tier (min priority among Pending or Running)
    let Some(current_tier) = current_priority_tier(state) else {
        return Command::None;
    };

    // Find next pending item AT the current tier only
    let next = state
        .queue_items
        .iter()
        .filter(|item| item.status == OperationStatus::Pending && item.priority == current_tier)
        .map(|item| item.id.clone())
        .next();

    if let Some(id) = next {
        // Mark as running immediately and set start time
        if let Some(item) = state.queue_items.iter_mut().find(|i| i.id == id) {
            item.status = OperationStatus::Running;
            item.started_at = Some(std::time::Instant::now());
            state.currently_running.insert(id.clone());
        }

        // Persist Running status to database
        let item_id_for_persist = id.clone();
        let persist_cmd = Command::perform(
            async move {
                let result = crate::global_config().update_queue_item_status(&item_id_for_persist, OperationStatus::Running).await;
                if let Err(e) = &result {
                    log::error!("Failed to persist Running status for {}: {}", item_id_for_persist, e);
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
            let exec_cmd = Command::perform(
                async move {
                    use crate::api::resilience::ResilienceConfig;
                    let start = std::time::Instant::now();
                    let op_count = item.operations.len();

                    // Get client for this environment from global client manager
                    let client = match crate::client_manager().get_client(&item.metadata.environment_name).await {
                        Ok(client) => client,
                        Err(e) => {
                            log::error!("Queue item {} - failed to get client: {}", item.id, e);
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
                    log::info!("Queue item {} - executing {} operations", item.id, op_count);
                    let result = item.operations.execute(&client, &resilience).await;
                    log::info!("Queue item {} - completed in {:?}", item.id, start.elapsed());
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
    } else {
        Command::None
    }
}
