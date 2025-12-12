use crossterm::event::KeyCode;
use std::path::PathBuf;
use crate::tui::{App, AppId, Command, Element, Subscription, Theme, FocusId, LayeredView, Resource};
use crate::tui::widgets::{FileBrowserState, FileBrowserEntry, FileBrowserEvent, SelectField, SelectEvent};
use ratatui::text::{Line, Span};
use ratatui::style::{Style, Stylize};

pub struct DeadlinesFileSelectApp;

#[derive(Clone)]
pub struct State {
    current_environment: Option<String>,
    file_browser_state: FileBrowserState,
    selected_file: Option<PathBuf>,
    available_sheets: Resource<Vec<String>>,
    sheet_selector: SelectField,
    board_of_directors_import: bool,
}

impl State {
    fn new(current_environment: Option<String>) -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut browser_state = FileBrowserState::new(current_dir);

        // Set filter to show only Excel files and directories
        browser_state.set_filter(is_excel_or_dir);

        // Auto-select first Excel file
        browser_state.select_first_matching(is_excel_file);

        Self {
            current_environment,
            file_browser_state: browser_state,
            selected_file: None,
            available_sheets: Resource::NotAsked,
            sheet_selector: SelectField::new(),
            board_of_directors_import: false,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new(None)
    }
}

#[derive(Clone)]
pub enum Msg {
    EnvironmentLoaded(Option<String>),
    FileSelected(PathBuf),
    SheetsLoaded(Result<Vec<String>, String>),
    DirectoryEntered(PathBuf),
    Navigate(KeyCode),
    SheetSelectorEvent(SelectEvent),
    ToggleBoardOfDirectors,
    ConfirmSelection,
    Back,
    SetViewportHeight(usize),
}

impl crate::tui::AppState for State {}

impl App for DeadlinesFileSelectApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: Self::InitParams) -> (State, Command<Msg>) {
        let state = State::new(None);
        let cmd = Command::batch(vec![
            Command::perform(
                async {
                    let manager = crate::client_manager();
                    manager.get_current_environment_name().await
                        .ok()
                        .flatten()
                },
                Msg::EnvironmentLoaded
            ),
            Command::set_focus(FocusId::new("file-browser")),
        ]);
        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::EnvironmentLoaded(env) => {
                state.current_environment = env;
                Command::None
            }
            Msg::FileSelected(path) => {
                state.selected_file = Some(path.clone());
                state.available_sheets = Resource::Loading;

                // Load Excel sheets asynchronously
                Command::perform(
                    async move {
                        load_excel_sheets(&path).await
                    },
                    Msg::SheetsLoaded
                )
            }
            Msg::SheetsLoaded(Ok(sheets)) => {
                if !sheets.is_empty() {
                    state.sheet_selector.state.update_option_count(sheets.len());
                    state.sheet_selector.state.select(0);
                    state.sheet_selector.set_value(Some(sheets[0].clone()));
                    state.available_sheets = Resource::Success(sheets);
                    Command::set_focus(FocusId::new("sheet-selector"))
                } else {
                    state.available_sheets = Resource::Failure("No sheets found in file".to_string());
                    Command::None
                }
            }
            Msg::SheetsLoaded(Err(err)) => {
                state.available_sheets = Resource::Failure(err);
                Command::None
            }
            Msg::DirectoryEntered(_path) => {
                // Auto-select first Excel file after entering directory
                state.file_browser_state.select_first_matching(is_excel_file);
                Command::None
            }
            Msg::Navigate(key) => {
                match key {
                    KeyCode::Up => {
                        state.file_browser_state.navigate_up();
                        Command::None
                    }
                    KeyCode::Down => {
                        state.file_browser_state.navigate_down();
                        Command::None
                    }
                    KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                        state.file_browser_state.handle_navigation_key(key);
                        Command::None
                    }
                    KeyCode::Enter => {
                        if let Some(action) = state.file_browser_state.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                crate::tui::widgets::FileBrowserAction::FileSelected(path) => {
                                    Command::batch(vec![Command::perform(
                                        async move { path },
                                        Msg::FileSelected
                                    )])
                                }
                                crate::tui::widgets::FileBrowserAction::DirectoryEntered(path) => {
                                    Command::batch(vec![Command::perform(
                                        async move { path },
                                        Msg::DirectoryEntered
                                    )])
                                }
                                _ => Command::None
                            }
                        } else {
                            Command::None
                        }
                    }
                    _ => Command::None
                }
            }
            Msg::SheetSelectorEvent(event) => {
                if let Resource::Success(ref sheets) = state.available_sheets {
                    let (cmd, selection_event) = state.sheet_selector.handle_event(event.clone(), sheets);

                    // Focus continue button after selecting a sheet
                    if selection_event.is_some() {
                        Command::batch(vec![
                            cmd,
                            Command::set_focus(FocusId::new("continue-button"))
                        ])
                    } else {
                        cmd
                    }
                } else {
                    Command::None
                }
            }
            Msg::ToggleBoardOfDirectors => {
                state.board_of_directors_import = !state.board_of_directors_import;
                Command::None
            }
            Msg::ConfirmSelection => {
                if let (Some(file_path), Resource::Success(_sheets)) = (&state.selected_file, &state.available_sheets) {
                    if let Some(sheet_name) = state.sheet_selector.value() {
                        return Command::batch(vec![
                            Command::start_app(
                                AppId::DeadlinesMapping,
                                super::models::MappingParams {
                                    file_path: file_path.clone(),
                                    sheet_name: sheet_name.to_string(),
                                    board_of_directors_import: state.board_of_directors_import,
                                }
                            ),
                            Command::quit_self(),
                        ]);
                    }
                }
                Command::None
            }
            Msg::Back => {
                Command::batch(vec![
                    Command::navigate_to(AppId::AppLauncher),
                    Command::quit_self(),
                ])
            }
            Msg::SetViewportHeight(height) => {
                let item_count = state.file_browser_state.entries().len();
                let list_state = state.file_browser_state.list_state_mut();
                list_state.set_viewport_height(height);
                list_state.update_scroll(height, item_count);
                Command::None
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        use crate::tui::element::LayoutConstraint::*;
        let theme = &crate::global_runtime_config().theme;
        use crate::{col, row, spacer};

        // File browser panel
        let browser = Element::file_browser("file-browser", &state.file_browser_state, theme)
            .on_file_selected(Msg::FileSelected)
            .on_directory_entered(Msg::DirectoryEntered)
            .on_navigate(Msg::Navigate)
            .on_render(Msg::SetViewportHeight)
            .build();

        let browser_panel = Element::panel(browser)
            .title(format!("Select Excel File - {}", state.file_browser_state.current_path().display()))
            .build();

        // Sheet selector panel content
        let sheet_content = match &state.available_sheets {
            Resource::NotAsked => {
                Element::styled_text(Line::from(vec![
                    Span::styled("Select an Excel file to view available sheets", Style::default().fg(theme.text_tertiary)),
                ])).build()
            }
            Resource::Loading => {
                Element::styled_text(Line::from(vec![
                    Span::styled("Loading sheets...", Style::default().fg(theme.accent_secondary)),
                ])).build()
            }
            Resource::Failure(err) => {
                Element::styled_text(Line::from(vec![
                    Span::styled(format!("Error: {}", err), Style::default().fg(theme.accent_error)),
                ])).build()
            }
            Resource::Success(sheets) => {
                let selector = Element::select("sheet-selector", sheets.clone(), &mut state.sheet_selector.state)
                    .on_event(Msg::SheetSelectorEvent)
                    .build();

                let selector_panel = Element::panel(selector)
                    .title("Sheet")
                    .build();

                col![
                    Element::styled_text(Line::from(vec![
                        Span::styled("Selected file: ", Style::default().fg(theme.text_tertiary)),
                        Span::styled(
                            state.selected_file.as_ref().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).unwrap_or_default(),
                            Style::default().fg(theme.accent_primary)
                        ),
                    ])).build() => Length(1),
                    spacer!() => Length(1),
                    selector_panel => Length(3),
                ]
            }
        };

        let sheet_panel = Element::panel(sheet_content)
            .title("Sheet Selection")
            .build();

        // Board of Directors checkbox (only visible when sheets are loaded)
        let has_sheets = matches!(state.available_sheets, Resource::Success(_));
        let checkbox_panel = if has_sheets {
            let checkbox = Element::checkbox("board-of-directors-checkbox", "Enable", state.board_of_directors_import)
                .on_toggle(Msg::ToggleBoardOfDirectors)
                .build();
            Element::panel(checkbox)
                .title("Board of Directors Import")
                .build()
        } else {
            Element::None
        };

        // Button row (only enabled when sheets are loaded)
        let button_row = if has_sheets {
            row![
                Element::button("back-button", "Back")
                    .on_press(Msg::Back)
                    .build(),
                spacer!(),
                Element::button("continue-button", "Continue")
                    .on_press(Msg::ConfirmSelection)
                    .build(),
            ]
        } else {
            row![
                Element::button("back-button", "Back")
                    .on_press(Msg::Back)
                    .build(),
                spacer!(),
                Element::button("continue-button", "Continue").build(),
            ]
        };

        // Main layout
        let main_content = col![
            browser_panel => Fill(1),
            sheet_panel => Length(7),
            checkbox_panel => Length(if has_sheets { 5 } else { 0 }),
            button_row => Length(3),
        ];

        let outer_panel = Element::panel(main_content)
            .title("Deadlines - File and Sheet Selection")
            .build();

        LayeredView::new(outer_panel)
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        vec![]
    }

    fn title() -> &'static str {
        "Deadlines - File Selection"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        state.current_environment.as_ref().map(|env| {
        let theme = &crate::global_runtime_config().theme;
            Line::from(vec![
                Span::styled("Environment: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(env.clone(), Style::default().fg(theme.accent_primary)),
            ])
        })
    }
}

/// Filter to show only Excel files and directories
fn is_excel_or_dir(entry: &FileBrowserEntry) -> bool {
    entry.is_dir || is_excel_file(entry)
}

/// Check if entry is an Excel file
fn is_excel_file(entry: &FileBrowserEntry) -> bool {
    if entry.is_dir {
        return false;
    }
    let name_lower = entry.name.to_lowercase();
    name_lower.ends_with(".xlsx") || name_lower.ends_with(".xls") || name_lower.ends_with(".xlsm")
}

/// Load sheet names from an Excel file
async fn load_excel_sheets(path: &PathBuf) -> Result<Vec<String>, String> {
    use calamine::{Reader, open_workbook, Xlsx};

    let workbook: Xlsx<_> = open_workbook(path)
        .map_err(|e| format!("Failed to open Excel file: {}", e))?;

    let sheets = workbook.sheet_names().to_vec();

    if sheets.is_empty() {
        Err("No sheets found in workbook".to_string())
    } else {
        Ok(sheets)
    }
}
