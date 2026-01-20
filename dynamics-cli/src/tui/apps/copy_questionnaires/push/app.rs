use super::super::copy::domain::Questionnaire;
use super::models::*;
use super::step_commands;
use super::view;
use crate::tui::{
    app::App,
    command::{AppId, Command},
    renderer::LayeredView,
    subscription::Subscription,
};
use crossterm::event::KeyCode;
use ratatui::text::Line;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct PushQuestionnaireApp;

/// Type alias for step command futures
type StepFuture = Pin<
    Box<
        dyn Future<Output = Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>>
            + Send,
    >,
>;

/// Generic handler for step completion messages (Steps 2-10)
/// Reduces code duplication by handling common logic
fn handle_step_complete<F>(
    state: &mut State,
    result: Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>,
    next_phase: CopyPhase,
    next_step: usize,
    update_progress: F,
    next_command: impl FnOnce(
        Arc<Questionnaire>,
        HashMap<String, String>,
        Vec<(String, String)>,
    ) -> StepFuture,
    next_msg: fn(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>) -> Msg,
) -> Command<Msg>
where
    F: FnOnce(&mut CopyProgress),
{
    match result {
        Ok((new_id_map, new_created_ids)) => {
            // Update state with new mappings
            state.id_map = new_id_map;
            state.created_ids = new_created_ids;

            // Update progress
            if let PushState::Copying(ref mut progress) = state.push_state {
                progress.phase = next_phase.clone();
                progress.step = next_step;
                update_progress(progress);
            }

            // Check if cancellation was requested
            if state.cancel_requested {
                log::info!("Copy cancelled by user after step {}", next_step - 1);
                let error = CopyError {
                    phase: next_phase,
                    step: next_step,
                    error_message: "Copy cancelled by user".to_string(),
                    partial_counts: HashMap::new(),
                    rollback_complete: false,
                    orphaned_entities_csv: None,
                };
                state.push_state = PushState::Failed(error);
                state.cancel_requested = false;
                let created_ids = state.created_ids.clone();
                return Command::perform(
                    super::step_commands::rollback_created_entities(created_ids),
                    Msg::RollbackComplete,
                );
            }

            // Start next step
            let questionnaire = Arc::clone(&state.questionnaire);
            let id_map = state.id_map.clone();
            let created_ids = state.created_ids.clone();

            Command::perform(next_command(questionnaire, id_map, created_ids), next_msg)
        }
        Err(error) => {
            state.push_state = PushState::Failed(error);
            // Trigger rollback of all created entities
            let created_ids = state.created_ids.clone();
            Command::perform(
                super::step_commands::rollback_created_entities(created_ids),
                Msg::RollbackComplete,
            )
        }
    }
}

impl crate::tui::AppState for State {}

impl App for PushQuestionnaireApp {
    type State = State;
    type Msg = Msg;
    type InitParams = PushQuestionnaireParams;

    fn init(params: PushQuestionnaireParams) -> (State, Command<Msg>) {
        let state = State {
            questionnaire_id: params.questionnaire_id,
            copy_name: params.copy_name,
            copy_code: params.copy_code,
            questionnaire: params.questionnaire,
            push_state: PushState::Confirming,
            id_map: HashMap::new(),
            created_ids: Vec::new(),
            classifications_associated: 0,
            start_time: None,
            cancel_requested: false,
            show_undo_confirmation: false,
        };

        (state, Command::None)
    }

    fn update(state: &mut Self::State, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            Msg::StartCopy => {
                log::info!("Starting copy operation");

                // Initialize state
                state.start_time = Some(std::time::Instant::now());
                state.id_map.clear();
                state.created_ids.clear();

                // Transition to copying state
                state.push_state = PushState::Copying(CopyProgress::new(&state.questionnaire));

                // Start Step 1
                let questionnaire = Arc::clone(&state.questionnaire);
                let copy_name = state.copy_name.clone();
                let copy_code = state.copy_code.clone();

                Command::perform(
                    super::step_commands::step1_create_questionnaire(
                        questionnaire,
                        copy_name,
                        copy_code,
                    ),
                    |result| Msg::Step1Complete(result.map(|(id, _)| id)),
                )
            }

            Msg::Step1Complete(result) => {
                match result {
                    Ok(new_q_id) => {
                        // Check if cancellation was requested
                        if state.cancel_requested {
                            log::info!("Copy cancelled by user after step 1");
                            let error = CopyError {
                                phase: CopyPhase::CreatingPages,
                                step: 2,
                                error_message: "Copy cancelled by user".to_string(),
                                partial_counts: HashMap::new(),
                                rollback_complete: false,
                                orphaned_entities_csv: None,
                            };
                            state.push_state = PushState::Failed(error);
                            state.cancel_requested = false;
                            let created_ids = state.created_ids.clone();
                            return Command::perform(
                                super::step_commands::rollback_created_entities(created_ids),
                                Msg::RollbackComplete,
                            );
                        }

                        // Update id map and created_ids
                        state
                            .id_map
                            .insert(state.questionnaire_id.clone(), new_q_id.clone());
                        state
                            .created_ids
                            .push(("nrq_questionnaires".to_string(), new_q_id));

                        // Update progress
                        if let PushState::Copying(ref mut progress) = state.push_state {
                            progress.phase = CopyPhase::CreatingPages;
                            progress.step = 2;
                            progress.complete(EntityType::Questionnaire);
                        }

                        // Start Step 2
                        let questionnaire = Arc::clone(&state.questionnaire);
                        let id_map = state.id_map.clone();
                        let created_ids = state.created_ids.clone();

                        Command::perform(
                            super::step_commands::step2_create_pages(
                                questionnaire,
                                id_map,
                                created_ids,
                            ),
                            Msg::Step2Complete, // Keep full tuple
                        )
                    }
                    Err(error) => {
                        state.push_state = PushState::Failed(error);
                        // Trigger rollback of all created entities
                        let created_ids = state.created_ids.clone();
                        Command::perform(
                            super::step_commands::rollback_created_entities(created_ids),
                            Msg::RollbackComplete,
                        )
                    }
                }
            }

            Msg::Step2Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingPageLines,
                3,
                |progress| {
                    progress.complete(EntityType::Pages);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step3_create_page_lines(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step3Complete,
            ),

            Msg::Step3Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingGroups,
                4,
                |progress| {
                    progress.complete(EntityType::PageLines);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step4_create_groups(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step4Complete,
            ),

            Msg::Step4Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingGroupLines,
                5,
                |progress| {
                    progress.complete(EntityType::Groups);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step5_create_group_lines(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step5Complete,
            ),

            Msg::Step5Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingQuestions,
                6,
                |progress| {
                    progress.complete(EntityType::GroupLines);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step6_create_questions(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step6Complete,
            ),

            Msg::Step6Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingTemplateLines,
                7,
                |progress| {
                    progress.complete(EntityType::Questions);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step7_create_template_lines(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step7Complete,
            ),

            Msg::Step7Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingConditions,
                8,
                |progress| {
                    progress.complete(EntityType::TemplateLines);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step8_create_conditions(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step8Complete,
            ),

            Msg::Step8Complete(result) => handle_step_complete(
                state,
                result,
                CopyPhase::CreatingConditionActions,
                9,
                |progress| {
                    progress.complete(EntityType::Conditions);
                },
                |q, id_map, created_ids| {
                    Box::pin(super::step_commands::step9_create_condition_actions(
                        q,
                        id_map,
                        created_ids,
                    ))
                },
                Msg::Step9Complete,
            ),

            Msg::Step9Complete(result) => {
                match result {
                    Ok((new_id_map, new_created_ids)) => {
                        state.id_map = new_id_map;
                        state.created_ids = new_created_ids;

                        // Update progress
                        if let PushState::Copying(progress) = &mut state.push_state {
                            progress.complete(EntityType::ConditionActions);
                            progress.phase = CopyPhase::CreatingClassifications;
                            progress.step = 10;
                        }

                        // Trigger step 10
                        let questionnaire = Arc::clone(&state.questionnaire);
                        let id_map = state.id_map.clone();
                        let created_ids = state.created_ids.clone();
                        Command::perform(
                            super::step_commands::step10_create_classifications(
                                questionnaire,
                                id_map,
                                created_ids,
                            ),
                            Msg::Step10Complete,
                        )
                    }
                    Err(error) => {
                        state.push_state = PushState::Failed(error);
                        // Trigger rollback of all created entities
                        let created_ids = state.created_ids.clone();
                        Command::perform(
                            super::step_commands::rollback_created_entities(created_ids),
                            Msg::RollbackComplete,
                        )
                    }
                }
            }

            Msg::Step10Complete(result) => {
                match result {
                    Ok((new_id_map, new_created_ids, classifications_count)) => {
                        state.id_map = new_id_map;
                        state.created_ids = new_created_ids;
                        state.classifications_associated = classifications_count;

                        // Update progress
                        if let PushState::Copying(progress) = &mut state.push_state {
                            progress.phase = CopyPhase::PublishingConditions;
                            progress.step = 11;
                            progress.complete(EntityType::Classifications);
                        }

                        // Trigger step 11
                        let questionnaire = Arc::clone(&state.questionnaire);
                        let id_map = state.id_map.clone();
                        let created_ids = state.created_ids.clone();
                        Command::perform(
                            super::step_commands::step11_publish_conditions(
                                questionnaire,
                                id_map,
                                created_ids,
                            ),
                            Msg::Step11Complete,
                        )
                    }
                    Err(error) => {
                        state.push_state = PushState::Failed(error);
                        // Trigger rollback of all created entities
                        let created_ids = state.created_ids.clone();
                        Command::perform(
                            super::step_commands::rollback_created_entities(created_ids),
                            Msg::RollbackComplete,
                        )
                    }
                }
            }

            Msg::Step11Complete(result) => {
                match result {
                    Ok((new_id_map, new_created_ids)) => {
                        state.id_map = new_id_map;
                        state.created_ids = new_created_ids;

                        // Complete condition status updates progress
                        if let PushState::Copying(progress) = &mut state.push_state {
                            progress.complete(EntityType::ConditionStatusUpdates);
                        }

                        // Calculate final statistics
                        let new_questionnaire_id = state
                            .id_map
                            .get(&state.questionnaire_id)
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());

                        // Map entity_set names to friendly names for UI display
                        let mut entities_created = HashMap::new();
                        for (entity_set, _) in &state.created_ids {
                            let friendly_name =
                                step_commands::entity_set_to_friendly_name(entity_set.as_str());
                            *entities_created
                                .entry(friendly_name.to_string())
                                .or_insert(0) += 1;
                        }

                        // Add classifications associated count (not in created_ids since they're associations, not entities)
                        if state.classifications_associated > 0 {
                            entities_created.insert(
                                "classifications".to_string(),
                                state.classifications_associated,
                            );
                        }

                        let total_entities =
                            state.created_ids.len() + state.classifications_associated;
                        let duration = state.start_time.map(|t| t.elapsed()).unwrap_or_default();

                        // Transition to success state
                        state.push_state = PushState::Success(CopyResult {
                            new_questionnaire_id,
                            new_questionnaire_name: state.copy_name.clone(),
                            entities_created,
                            total_entities,
                            duration,
                        });

                        Command::None
                    }
                    Err(error) => {
                        state.push_state = PushState::Failed(error);
                        // Trigger rollback of all created entities
                        let created_ids = state.created_ids.clone();
                        Command::perform(
                            super::step_commands::rollback_created_entities(created_ids),
                            Msg::RollbackComplete,
                        )
                    }
                }
            }

            Msg::CopySuccess(result) => {
                // Copy completed successfully
                state.push_state = PushState::Success(result);
                Command::None
            }

            Msg::CopyFailed(error) => {
                // Copy failed - already rolled back by step error handler
                state.push_state = PushState::Failed(error);
                Command::None
            }

            Msg::RollbackComplete(result) => {
                // Update the error state with rollback status
                if let PushState::Failed(ref mut error) = state.push_state {
                    match result {
                        Ok(_) => {
                            error.rollback_complete = true;
                            log::info!("Rollback completed successfully");
                        }
                        Err(csv_path) => {
                            error.rollback_complete = false;
                            error.orphaned_entities_csv = Some(csv_path.clone());
                            log::error!(
                                "Rollback failed - orphaned entities exported to: {}",
                                csv_path
                            );
                        }
                    }
                }
                Command::None
            }

            Msg::Cancel | Msg::Back => Command::batch(vec![
                Command::navigate_to(AppId::CopyQuestionnaire),
                Command::quit_self(),
            ]),

            Msg::Done => {
                // Success - go back to questionnaire list
                Command::batch(vec![
                    Command::navigate_to(AppId::SelectQuestionnaire),
                    Command::quit_self(),
                ])
            }

            Msg::Retry => {
                // Reset to confirmation screen
                state.push_state = PushState::Confirming;
                Command::None
            }

            Msg::ViewCopy => {
                // TODO: Navigate to view the newly created questionnaire
                log::info!("View copy (not implemented)");
                Command::None
            }

            Msg::CopyAnother => {
                // Go back to select questionnaire screen
                Command::batch(vec![
                    Command::navigate_to(AppId::SelectQuestionnaire),
                    Command::quit_self(),
                ])
            }

            Msg::UndoCopy => {
                // Show confirmation modal before undo
                log::info!("Showing undo confirmation");
                state.show_undo_confirmation = true;
                Command::None
            }

            Msg::ConfirmUndo => {
                // User confirmed undo - trigger rollback
                log::info!("User confirmed undo of successful copy");
                state.show_undo_confirmation = false;

                // Transition to Failed state to show rollback progress
                // (Quick hack: reuse the failure UI which has rollback tracking)
                if let PushState::Success(result) = &state.push_state {
                    let synthetic_error = CopyError {
                        phase: CopyPhase::CreatingClassifications, // Last phase
                        step: 10,
                        error_message: "Manual undo requested by user".to_string(),
                        partial_counts: result.entities_created.clone(),
                        rollback_complete: false, // Will be set by RollbackComplete
                        orphaned_entities_csv: None,
                    };
                    state.push_state = PushState::Failed(synthetic_error);
                }

                let created_ids = state.created_ids.clone();
                Command::perform(
                    super::step_commands::rollback_created_entities(created_ids),
                    Msg::RollbackComplete,
                )
            }

            Msg::CancelUndo => {
                // User cancelled undo - hide confirmation
                log::info!("User cancelled undo");
                state.show_undo_confirmation = false;
                Command::None
            }

            Msg::CancelCopy => {
                // User pressed Esc during copy - set cancellation flag
                log::info!("User requested copy cancellation");
                state.cancel_requested = true;
                // The actual cancellation will happen when the current operation completes
                // and the next step handler checks this flag
                Command::None
            }
        }
    }

    fn view(state: &mut Self::State) -> LayeredView<Self::Msg> {
        view::render_view(state)
    }

    fn subscriptions(state: &Self::State) -> Vec<Subscription<Self::Msg>> {
        match &state.push_state {
            PushState::Confirming => {
                vec![
                    Subscription::keyboard(KeyCode::Enter, "Start Copy", Msg::StartCopy),
                    Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::Cancel),
                ]
            }
            PushState::Copying(_) => {
                vec![Subscription::keyboard(
                    KeyCode::Esc,
                    "Cancel (will rollback)",
                    Msg::CancelCopy,
                )]
            }
            PushState::Success(_) => {
                if state.show_undo_confirmation {
                    // Show y/n confirmation keys
                    vec![
                        Subscription::keyboard(
                            KeyCode::Char('y'),
                            "Yes, delete all",
                            Msg::ConfirmUndo,
                        ),
                        Subscription::keyboard(KeyCode::Char('n'), "No, keep it", Msg::CancelUndo),
                        Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::CancelUndo),
                    ]
                } else {
                    // Normal success screen keys
                    vec![
                        Subscription::keyboard(KeyCode::Enter, "Done", Msg::Done),
                        Subscription::keyboard(KeyCode::Char('u'), "Undo Copy", Msg::UndoCopy),
                        Subscription::keyboard(
                            KeyCode::Char('c'),
                            "Copy Another",
                            Msg::CopyAnother,
                        ),
                        Subscription::keyboard(KeyCode::Char('v'), "View Copy", Msg::ViewCopy),
                    ]
                }
            }
            PushState::Failed(_) => {
                vec![
                    Subscription::keyboard(KeyCode::Char('r'), "Retry", Msg::Retry),
                    Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::Cancel),
                ]
            }
        }
    }

    fn title() -> &'static str {
        "Push Questionnaire"
    }

    fn status(state: &Self::State) -> Option<Line<'static>> {
        view::render_status(state)
    }
}
