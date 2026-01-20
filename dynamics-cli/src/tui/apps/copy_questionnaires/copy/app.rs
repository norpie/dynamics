use super::data_loading::{build_domain_model, load_full_snapshot};
use super::models::*;
use super::view;
use crate::tui::{
    Resource,
    app::App,
    command::{AppId, Command},
    renderer::LayeredView,
    subscription::Subscription,
};
use crossterm::event::KeyCode;
use ratatui::text::Line;

/// Validate copy parameters before starting the copy operation
fn validate_copy_params(copy_name: &str, copy_code: &str) -> Result<(), String> {
    // Validate copy name
    let trimmed_name = copy_name.trim();
    if trimmed_name.is_empty() {
        return Err("Copy name cannot be empty".to_string());
    }
    if trimmed_name.len() > 200 {
        return Err("Copy name too long (max 200 characters)".to_string());
    }

    // Validate copy code
    let trimmed_code = copy_code.trim();
    if trimmed_code.is_empty() {
        return Err("Copy code cannot be empty".to_string());
    }
    if trimmed_code.len() > 50 {
        return Err("Copy code too long (max 50 characters)".to_string());
    }

    // Check for invalid characters in copy code (alphanumeric, dash, underscore only)
    if !trimmed_code
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Copy code can only contain letters, numbers, dashes, and underscores".to_string(),
        );
    }

    Ok(())
}

pub struct CopyQuestionnaireApp;

impl crate::tui::AppState for State {}

impl App for CopyQuestionnaireApp {
    type State = State;
    type Msg = Msg;
    type InitParams = CopyQuestionnaireParams;

    fn init(params: CopyQuestionnaireParams) -> (State, Command<Msg>) {
        let default_copy_name = format!("{} - Copy", params.questionnaire_name);
        let mut copy_name_input = crate::tui::widgets::fields::TextInputField::new();
        copy_name_input.set_value(default_copy_name);

        let mut state = State {
            questionnaire_id: params.questionnaire_id.clone(),
            questionnaire_name: params.questionnaire_name,
            questionnaire: Resource::Loading,
            tree_state: crate::tui::widgets::TreeState::with_selection(),
            copy_name_input,
            copy_code_input: crate::tui::widgets::fields::TextInputField::new(),
            validation_error: None,
        };

        // Load complete questionnaire snapshot - single task that loads everything sequentially
        let questionnaire_id = params.questionnaire_id.clone();
        let cmd = Command::perform_parallel()
            .add_task("Loading questionnaire structure", async move {
                let snapshot = load_full_snapshot(&questionnaire_id).await?;
                build_domain_model(snapshot)
            })
            .with_title("Loading Questionnaire Data")
            .on_complete(AppId::CopyQuestionnaire)
            .build(|_task_idx, result| {
                let data = result
                    .downcast::<Result<super::domain::Questionnaire, String>>()
                    .unwrap();
                Msg::QuestionnaireLoaded(*data)
            });

        (state, cmd)
    }

    fn update(state: &mut Self::State, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            Msg::QuestionnaireLoaded(result) => {
                match result {
                    Ok(questionnaire) => {
                        log::info!(
                            "Successfully loaded questionnaire with {} total entities",
                            questionnaire.total_entities()
                        );

                        // Extract copypostfix from raw questionnaire data and populate copy_code_input
                        if let Some(copypostfix) = questionnaire
                            .raw
                            .get("nrq_copypostfix")
                            .and_then(|v| v.as_str())
                        {
                            state.copy_code_input.set_value(copypostfix.to_string());
                            log::debug!("Set copy code to: {}", copypostfix);
                        } else {
                            log::debug!("No copypostfix found in questionnaire");
                        }

                        state.questionnaire = Resource::Success(questionnaire);
                    }
                    Err(e) => {
                        log::error!("Failed to load questionnaire: {}", e);
                        state.questionnaire = Resource::Failure(e);
                    }
                }
                Command::None
            }
            Msg::TreeEvent(event) => {
                state.tree_state.handle_event(event);
                Command::None
            }
            Msg::TreeNodeClicked(node_id) => {
                // Handle tree node clicks (e.g., select node, expand/collapse)
                log::debug!("Tree node clicked: {}", node_id);
                // TODO: Add specific behavior when nodes are clicked
                Command::None
            }
            Msg::ViewportHeight(height) => {
                // Renderer provides viewport height for proper scrolling
                state.tree_state.set_viewport_height(height);
                Command::None
            }
            Msg::CopyNameInputEvent(event) => {
                state.copy_name_input.handle_event(event, None);
                // Clear validation error when user edits the input
                state.validation_error = None;
                Command::None
            }
            Msg::CopyCodeInputEvent(event) => {
                state.copy_code_input.handle_event(event, None);
                // Clear validation error when user edits the input
                state.validation_error = None;
                Command::None
            }
            Msg::Continue => {
                // Validate inputs before continuing
                let copy_name = state.copy_name_input.value();
                let copy_code = state.copy_code_input.value();

                if let Err(error) = validate_copy_params(copy_name, copy_code) {
                    log::warn!("Validation failed: {}", error);
                    state.validation_error = Some(error);
                    return Command::None;
                }

                // Navigate to push app with copy parameters
                log::info!(
                    "Continuing to push app with copy name: {} and copy code: {}",
                    copy_name,
                    copy_code
                );

                // Extract the questionnaire from state
                let questionnaire = match &state.questionnaire {
                    Resource::Success(q) => std::sync::Arc::new(q.clone()),
                    _ => {
                        log::error!("Cannot continue: questionnaire not loaded");
                        return Command::None;
                    }
                };

                // Clear validation error on successful validation
                state.validation_error = None;

                Command::start_app(
                    AppId::PushQuestionnaire,
                    crate::tui::apps::copy_questionnaires::push::PushQuestionnaireParams {
                        questionnaire_id: state.questionnaire_id.clone(),
                        copy_name: copy_name.to_string(),
                        copy_code: copy_code.to_string(),
                        questionnaire,
                    },
                )
            }
            Msg::Back => Command::batch(vec![
                Command::navigate_to(AppId::SelectQuestionnaire),
                Command::quit_self(),
            ]),
        }
    }

    fn view(state: &mut Self::State) -> LayeredView<Self::Msg> {
        view::render_view(state)
    }

    fn subscriptions(_state: &Self::State) -> Vec<Subscription<Self::Msg>> {
        vec![
            Subscription::keyboard(KeyCode::Esc, "Back to selection", Msg::Back),
            // TODO: Add keybinding for starting copy when implemented
            // Subscription::keyboard(KeyCode::Char('c'), "Start copy", Msg::StartCopy),
        ]
    }

    fn title() -> &'static str {
        "Copy Questionnaire"
    }

    fn status(state: &Self::State) -> Option<Line<'static>> {
        view::render_status(state)
    }
}
