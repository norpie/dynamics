use crate::tui::widgets::TextInputState;
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
    filtered_questionnaires: Vec<QuestionnaireItem>,
    list_state: ListState,
    search_text: String,
    search_input_state: TextInputState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            questionnaires: Resource::NotAsked,
            filtered_questionnaires: Vec::new(),
            list_state: ListState::with_selection(),
            search_text: String::new(),
            search_input_state: TextInputState::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuestionnaireItem {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub type_value: Option<i32>,
    pub publish_date: Option<String>,
    pub status_code: Option<i32>,
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

        // Build main name and code
        let mut spans = vec![];

        // Name and code
        let name_text = if let Some(code) = &self.code {
            format!("  {} ({})", self.name, code)
        } else {
            format!("  {}", self.name)
        };
        spans.push(Span::styled(name_text, Style::default().fg(fg_color)));

        // Add type badge
        if let Some(type_val) = self.type_value {
            let type_text = match type_val {
                170590000 => "Project",
                170590001 => "Request",
                _ => "Unknown",
            };
            let type_label = format!(" [{}]", type_text);
            spans.push(Span::styled(
                type_label,
                Style::default().fg(theme.accent_secondary),
            ));
        }

        // Add publish date badge
        if let Some(date) = &self.publish_date {
            // Parse and format date (expecting ISO format like "2024-01-15T00:00:00Z")
            let formatted_date = if let Some(date_part) = date.split('T').next() {
                date_part.to_string()
            } else {
                date.clone()
            };
            let date_label = format!(" [Published: {}]", formatted_date);
            spans.push(Span::styled(
                date_label,
                Style::default().fg(theme.text_secondary),
            ));
        }

        // Add status badge
        if let Some(status) = self.status_code {
            let (status_text, status_color) = match status {
                1 => ("Draft", theme.text_secondary),
                170590001 => ("Preview", theme.accent_warning),
                2 => ("Inactive", theme.text_secondary),
                170590002 => ("Published", theme.accent_success),
                _ => ("Unknown", theme.text_secondary),
            };
            let status_label = format!(" [{}]", status_text);
            spans.push(Span::styled(
                status_label,
                Style::default().fg(status_color),
            ));
        }

        let mut builder = Element::styled_text(Line::from(spans));

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
    SearchTextChanged(String),
    SearchInputEvent(crate::tui::widgets::TextInputEvent),
}

impl crate::tui::AppState for State {}

impl SelectQuestionnaireApp {
    /// Update filtered list based on search text using fuzzy matching
    fn update_filtered_list(state: &mut State) {
        if let Resource::Success(all_questionnaires) = &state.questionnaires {
            if state.search_text.is_empty() {
                // No search - show all questionnaires
                state.filtered_questionnaires = all_questionnaires.clone();
            } else {
                // Apply fuzzy matching
                use fuzzy_matcher::FuzzyMatcher;
                use fuzzy_matcher::skim::SkimMatcherV2;

                let matcher = SkimMatcherV2::default();
                let search_lower = state.search_text.to_lowercase();

                // Score each questionnaire
                let mut scored: Vec<(QuestionnaireItem, i64)> = all_questionnaires
                    .iter()
                    .filter_map(|q| {
                        // Build searchable text (name + code + type)
                        let mut searchable = q.name.clone();
                        if let Some(code) = &q.code {
                            searchable.push(' ');
                            searchable.push_str(code);
                        }
                        if let Some(type_val) = q.type_value {
                            let type_text = match type_val {
                                170590000 => " Project",
                                170590001 => " Request",
                                _ => "",
                            };
                            searchable.push_str(type_text);
                        }

                        // Try fuzzy match
                        matcher
                            .fuzzy_match(&searchable, &search_lower)
                            .map(|score| (q.clone(), score))
                    })
                    .collect();

                // Sort by score descending (higher score = better match)
                scored.sort_by(|a, b| b.1.cmp(&a.1));

                state.filtered_questionnaires = scored.into_iter().map(|(q, _)| q).collect();
            }

            // Reset list selection to first item
            let item_count = state.filtered_questionnaires.len();
            if item_count > 0 {
                state.list_state.select_and_scroll(Some(0), item_count);
            } else {
                state.list_state.select_and_scroll(None, 0);
            }
        }
    }
}

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
                            let type_value = item
                                .get("nrq_type")
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);
                            let publish_date = item
                                .get("nrq_publishdate")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let status_code = item
                                .get("statuscode")
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);
                            Some(QuestionnaireItem {
                                id,
                                name,
                                code,
                                type_value,
                                publish_date,
                                status_code,
                            })
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
                    "nrq_type".to_string(),
                    "nrq_publishdate".to_string(),
                    "statuscode".to_string(),
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
                        let type_value = item
                            .get("nrq_type")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32);
                        let publish_date = item
                            .get("nrq_publishdate")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let status_code = item
                            .get("statuscode")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32);
                        Some(QuestionnaireItem {
                            id,
                            name,
                            code,
                            type_value,
                            publish_date,
                            status_code,
                        })
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
                        state.questionnaires = Resource::Success(questionnaires);
                        Self::update_filtered_list(state);
                        if has_items {
                            return Command::set_focus(FocusId::new("search-input"));
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
                let visible_height = 20;
                state.list_state.handle_key(
                    key,
                    state.filtered_questionnaires.len(),
                    visible_height,
                );
                Command::None
            }
            Msg::SelectQuestionnaire => {
                if let Some(selected_idx) = state.list_state.selected() {
                    if let Some(questionnaire) = state.filtered_questionnaires.get(selected_idx) {
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
                            "nrq_type".to_string(),
                            "nrq_publishdate".to_string(),
                            "statuscode".to_string(),
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
                                let type_value = item
                                    .get("nrq_type")
                                    .and_then(|v| v.as_i64())
                                    .map(|v| v as i32);
                                let publish_date = item
                                    .get("nrq_publishdate")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                                let status_code = item
                                    .get("statuscode")
                                    .and_then(|v| v.as_i64())
                                    .map(|v| v as i32);
                                Some(QuestionnaireItem {
                                    id,
                                    name,
                                    code,
                                    type_value,
                                    publish_date,
                                    status_code,
                                })
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
            Msg::SearchTextChanged(new_text) => {
                state.search_text = new_text;
                Self::update_filtered_list(state);
                Command::None
            }
            Msg::SearchInputEvent(event) => {
                use crate::tui::widgets::TextInputEvent;
                match event {
                    TextInputEvent::Changed(key) => {
                        if let Some(new_value) =
                            state
                                .search_input_state
                                .handle_key(key, &state.search_text, None)
                        {
                            return Self::update(state, Msg::SearchTextChanged(new_value));
                        }
                    }
                    TextInputEvent::Submit => {
                        // On Enter, move focus to list if there are results
                        if !state.filtered_questionnaires.is_empty() {
                            return Command::set_focus(FocusId::new("questionnaire-list"));
                        }
                    }
                }
                Command::None
            }
        }
    }

    fn view(state: &mut Self::State) -> LayeredView<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let content = match &state.questionnaires {
            Resource::Success(_all_questionnaires) => {
                // Search input at the top
                let search_input = Element::text_input(
                    FocusId::new("search-input"),
                    &state.search_text,
                    &state.search_input_state,
                )
                .placeholder("Search questionnaires (name, code, type)...")
                .on_event(Msg::SearchInputEvent)
                .build();

                let search_panel = Element::panel(search_input).title("Search").build();

                // List of filtered questionnaires
                let list_content = if state.filtered_questionnaires.is_empty() {
                    if state.search_text.is_empty() {
                        Element::text("No questionnaires found in this environment")
                    } else {
                        Element::text(format!("No questionnaires match '{}'", state.search_text))
                    }
                } else {
                    Element::list(
                        FocusId::new("questionnaire-list"),
                        &state.filtered_questionnaires,
                        &state.list_state,
                        theme,
                    )
                    .on_select(|_| Msg::SelectQuestionnaire)
                    .on_activate(|_| Msg::SelectQuestionnaire)
                    .on_navigate(Msg::ListNavigate)
                    .build()
                };

                let list_panel = Element::panel(list_content)
                    .title(format!(
                        "Questionnaires ({}/{})",
                        state.filtered_questionnaires.len(),
                        _all_questionnaires.len()
                    ))
                    .build();

                // Stack search and list vertically
                Element::column(vec![search_panel, list_panel]).build()
            }
            Resource::Failure(err) => {
                Element::text(format!("Error loading questionnaires: {}", err))
            }
            _ => {
                // Loading or NotAsked states - LoadingScreen handles this
                Element::text("")
            }
        };

        LayeredView::new(content)
    }

    fn subscriptions(state: &Self::State) -> Vec<Subscription<Self::Msg>> {
        let mut subs = vec![
            Subscription::keyboard(KeyCode::Esc, "Back to app launcher", Msg::Back),
            Subscription::keyboard(KeyCode::F(5), "Refresh questionnaires", Msg::Refresh),
        ];

        // Only add Enter if we have filtered questionnaires
        if !state.filtered_questionnaires.is_empty() {
            subs.push(Subscription::keyboard(
                KeyCode::Enter,
                "Select questionnaire",
                Msg::SelectQuestionnaire,
            ));
        }

        subs
    }

    fn title() -> &'static str {
        "Copy Questionnaire - Select"
    }

    fn status(state: &Self::State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;

        match &state.questionnaires {
            Resource::Success(all_questionnaires) => {
                let status_text = if state.search_text.is_empty() {
                    format!("{} questionnaires", all_questionnaires.len())
                } else {
                    format!(
                        "{} of {} questionnaires (filtered)",
                        state.filtered_questionnaires.len(),
                        all_questionnaires.len()
                    )
                };
                Some(Line::from(vec![Span::styled(
                    status_text,
                    Style::default().fg(theme.text_primary),
                )]))
            }
            _ => None,
        }
    }
}
