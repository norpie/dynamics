use crate::tui::widgets::list::{ListItem, ListState};
use crate::tui::{
    Resource,
    app::App,
    command::{AppId, Command},
    element::{Element, FocusId},
    renderer::LayeredView,
    subscription::Subscription,
};
use crossterm::event::KeyCode;
use ratatui::{
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
};
use serde_json::Value;

pub struct SelectQuestionnaireApp;

#[derive(Clone)]
pub struct State {
    questionnaires: Resource<Vec<QuestionnaireItem>>,
    list_state: ListState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            questionnaires: Resource::NotAsked,
            list_state: ListState::with_selection(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuestionnaireItem {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
}

impl ListItem for QuestionnaireItem {
    type Msg = Msg;

    fn to_element(
        &self,
        is_selected: bool,
        _is_multi_selected: bool,
        _is_hovered: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (
                theme.accent_primary,
                Some(Style::default().bg(theme.bg_surface)),
            )
        } else {
            (theme.text_primary, None)
        };

        let display_text = if let Some(code) = &self.code {
            format!("  {} ({})", self.name, code)
        } else {
            format!("  {}", self.name)
        };

        let mut builder = Element::styled_text(Line::from(vec![Span::styled(
            display_text,
            Style::default().fg(fg_color),
        )]));

        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }

        builder.build()
    }
}

#[derive(Clone)]
pub enum Msg {
    QuestionnairesLoaded(Result<Vec<QuestionnaireItem>, String>),
    ListNavigate(KeyCode),
    SelectQuestionnaire,
    Refresh,
    Back,
}

impl crate::tui::AppState for State {}

impl App for SelectQuestionnaireApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: ()) -> (State, Command<Msg>) {
        let mut state = State::default();
        state.questionnaires = Resource::Loading;

        // Use LoadingScreen with parallel task execution
        let cmd = Command::perform_parallel()
            .add_task("Loading questionnaires", async {
                // Get current environment
                let manager = crate::client_manager();
                let env_name = manager
                    .get_current_environment_name()
                    .await
                    .ok()
                    .flatten()
                    .ok_or_else(|| "No environment selected".to_string())?;

                // Try to get from cache first (12 hours)
                let config = crate::global_config();
                let entity_name = "nrq_questionnaire";

                if let Ok(Some(cached_data)) = config
                    .get_entity_data_cache(&env_name, entity_name, 12)
                    .await
                {
                    // Parse cached data back to QuestionnaireItem vec
                    let questionnaires: Vec<QuestionnaireItem> = cached_data
                        .iter()
                        .filter_map(|item| {
                            let id = item.get("nrq_questionnaireid")?.as_str()?.to_string();
                            let name = item.get("nrq_name")?.as_str()?.to_string();
                            let code = item
                                .get("nrq_code")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            Some(QuestionnaireItem { id, name, code })
                        })
                        .collect();
                    log::info!("Loaded {} questionnaires from cache", questionnaires.len());
                    return Ok::<Vec<QuestionnaireItem>, String>(questionnaires);
                }

                // Fetch from API
                log::info!("Fetching questionnaires from Dynamics 365");
                let client = manager
                    .get_client(&env_name)
                    .await
                    .map_err(|e| e.to_string())?;

                // Build query for questionnaires
                use crate::api::query::{Filter, OrderBy, Query};
                let mut query = Query::new("nrq_questionnaires");
                query.select = Some(vec![
                    "nrq_questionnaireid".to_string(),
                    "nrq_name".to_string(),
                    "nrq_code".to_string(),
                ]);
                // Only show Active questionnaires (statecode = 0) to avoid copying incomplete/orphaned data
                query.filter = Some(Filter::eq("statecode", 0));
                query.orderby = query.orderby.add(OrderBy::asc("nrq_name"));

                let result = client
                    .execute_query(&query)
                    .await
                    .map_err(|e| e.to_string())?;

                let data_response = result
                    .data
                    .ok_or_else(|| "No data in response".to_string())?;

                let value_vec = data_response.value;

                let questionnaires: Vec<QuestionnaireItem> = value_vec
                    .iter()
                    .filter_map(|item| {
                        let id = item.get("nrq_questionnaireid")?.as_str()?.to_string();
                        let name = item.get("nrq_name")?.as_str()?.to_string();
                        let code = item
                            .get("nrq_code")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        Some(QuestionnaireItem { id, name, code })
                    })
                    .collect();

                log::info!("Loaded {} questionnaires from API", questionnaires.len());

                // Cache the results
                let _ = config
                    .set_entity_data_cache(&env_name, entity_name, &value_vec)
                    .await;

                Ok::<Vec<QuestionnaireItem>, String>(questionnaires)
            })
            .with_title("Loading Questionnaires")
            .on_complete(AppId::SelectQuestionnaire)
            .build(|_task_idx, result| {
                let data = result
                    .downcast::<Result<Vec<QuestionnaireItem>, String>>()
                    .unwrap();
                Msg::QuestionnairesLoaded(*data)
            });

        (state, cmd)
    }

    fn update(state: &mut Self::State, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            Msg::QuestionnairesLoaded(result) => {
                match result {
                    Ok(questionnaires) => {
                        let has_items = !questionnaires.is_empty();
                        let item_count = questionnaires.len();
                        state.questionnaires = Resource::Success(questionnaires);
                        if has_items {
                            state.list_state.select_and_scroll(Some(0), item_count);
                            return Command::set_focus(FocusId::new("questionnaire-list"));
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to load questionnaires: {}", e);
                        state.questionnaires = Resource::Failure(e);
                    }
                }
                Command::None
            }
            Msg::ListNavigate(key) => {
                if let Resource::Success(questionnaires) = &state.questionnaires {
                    let visible_height = 20;
                    state
                        .list_state
                        .handle_key(key, questionnaires.len(), visible_height);
                }
                Command::None
            }
            Msg::SelectQuestionnaire => {
                if let Resource::Success(questionnaires) = &state.questionnaires {
                    if let Some(selected_idx) = state.list_state.selected() {
                        if let Some(questionnaire) = questionnaires.get(selected_idx) {
                            log::info!(
                                "Selected questionnaire: {} ({})",
                                questionnaire.name,
                                questionnaire.id
                            );

                            // Navigate to copy app
                            let params = super::copy::CopyQuestionnaireParams {
                                questionnaire_id: questionnaire.id.clone(),
                                questionnaire_name: questionnaire.name.clone(),
                            };

                            return Command::batch(vec![
                                Command::start_app(AppId::CopyQuestionnaire, params),
                                Command::quit_self(),
                            ]);
                        }
                    }
                }
                Command::None
            }
            Msg::Refresh => {
                // Clear cache and re-fetch questionnaires
                log::info!("Refreshing questionnaires list");
                state.questionnaires = Resource::Loading;

                let cmd = Command::perform_parallel()
                    .add_task("Refreshing questionnaires", async {
                        // Get current environment
                        let manager = crate::client_manager();
                        let env_name = manager
                            .get_current_environment_name()
                            .await
                            .ok()
                            .flatten()
                            .ok_or_else(|| "No environment selected".to_string())?;

                        let entity_name = "nrq_questionnaire";

                        // Clear cache to force refresh
                        let config = crate::global_config();
                        let _ = config
                            .delete_entity_data_cache(&env_name, entity_name)
                            .await;

                        // Fetch from API
                        log::info!("Fetching questionnaires from Dynamics 365 (bypassing cache)");
                        let client = manager
                            .get_client(&env_name)
                            .await
                            .map_err(|e| e.to_string())?;

                        // Build query for questionnaires
                        use crate::api::query::{Filter, OrderBy, Query};
                        let mut query = Query::new("nrq_questionnaires");
                        query.select = Some(vec![
                            "nrq_questionnaireid".to_string(),
                            "nrq_name".to_string(),
                            "nrq_code".to_string(),
                        ]);
                        // Only show Active questionnaires (statecode = 0) to avoid copying incomplete/orphaned data
                        query.filter = Some(Filter::eq("statecode", 0));
                        query.orderby = query.orderby.add(OrderBy::asc("nrq_name"));

                        let result = client
                            .execute_query(&query)
                            .await
                            .map_err(|e| e.to_string())?;

                        let data_response = result
                            .data
                            .ok_or_else(|| "No data in response".to_string())?;

                        let value_vec = data_response.value;

                        let questionnaires: Vec<QuestionnaireItem> = value_vec
                            .iter()
                            .filter_map(|item| {
                                let id = item.get("nrq_questionnaireid")?.as_str()?.to_string();
                                let name = item.get("nrq_name")?.as_str()?.to_string();
                                let code = item
                                    .get("nrq_code")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                                Some(QuestionnaireItem { id, name, code })
                            })
                            .collect();

                        log::info!("Refreshed {} questionnaires from API", questionnaires.len());

                        // Cache the results
                        let _ = config
                            .set_entity_data_cache(&env_name, entity_name, &value_vec)
                            .await;

                        Ok::<Vec<QuestionnaireItem>, String>(questionnaires)
                    })
                    .with_title("Refreshing Questionnaires")
                    .on_complete(AppId::SelectQuestionnaire)
                    .build(|_task_idx, result| {
                        let data = result
                            .downcast::<Result<Vec<QuestionnaireItem>, String>>()
                            .unwrap();
                        Msg::QuestionnairesLoaded(*data)
                    });

                cmd
            }
            Msg::Back => Command::batch(vec![
                Command::navigate_to(AppId::AppLauncher),
                Command::quit_self(),
            ]),
        }
    }

    fn view(state: &mut Self::State) -> LayeredView<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let content = match &state.questionnaires {
            Resource::Success(questionnaires) if questionnaires.is_empty() => {
                Element::text("No questionnaires found in this environment")
            }
            Resource::Success(questionnaires) => Element::list(
                FocusId::new("questionnaire-list"),
                questionnaires,
                &state.list_state,
                theme,
            )
            .on_select(|_| Msg::SelectQuestionnaire)
            .on_activate(|_| Msg::SelectQuestionnaire)
            .on_navigate(Msg::ListNavigate)
            .build(),
            Resource::Failure(err) => {
                Element::text(format!("Error loading questionnaires: {}", err))
            }
            _ => {
                // Loading or NotAsked states - LoadingScreen handles this
                Element::text("")
            }
        };

        let panel = Element::panel(content)
            .title("Select Questionnaire to Copy")
            .build();

        LayeredView::new(panel)
    }

    fn subscriptions(state: &Self::State) -> Vec<Subscription<Self::Msg>> {
        let mut subs = vec![
            Subscription::keyboard(KeyCode::Esc, "Back to app launcher", Msg::Back),
            Subscription::keyboard(KeyCode::F(5), "Refresh questionnaires", Msg::Refresh),
        ];

        // Only add Enter if we have questionnaires loaded
        if let Resource::Success(questionnaires) = &state.questionnaires {
            if !questionnaires.is_empty() {
                subs.push(Subscription::keyboard(
                    KeyCode::Enter,
                    "Select questionnaire",
                    Msg::SelectQuestionnaire,
                ));
            }
        }

        subs
    }

    fn title() -> &'static str {
        "Copy Questionnaire - Select"
    }

    fn status(state: &Self::State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;

        match &state.questionnaires {
            Resource::Success(questionnaires) => Some(Line::from(vec![Span::styled(
                format!("{} questionnaires", questionnaires.len()),
                Style::default().fg(theme.text_primary),
            )])),
            _ => None,
        }
    }
}
