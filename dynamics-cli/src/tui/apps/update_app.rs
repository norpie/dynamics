//! Update TUI application for managing software updates

use crate::tui::{
    LayeredView, Resource,
    app::App,
    command::Command,
    element::{Element, FocusId},
    subscription::Subscription,
};
use crate::{col, spacer};
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct State {
    // Update information
    current_version: String,
    update_info: Resource<crate::update::UpdateInfo>,

    // Auto-update settings
    auto_check_enabled: bool,
    auto_install_enabled: bool,

    // Last check time
    last_check_time: Option<chrono::DateTime<chrono::Utc>>,

    // Installation state
    installing: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    // Check for updates
    CheckForUpdates,
    UpdateInfoLoaded(Result<crate::update::UpdateInfo, String>),

    // Install update
    InstallUpdate,
    UpdateInstalled(Result<String, String>),

    // Timer tick (every hour)
    TimerTick,

    // Load initial state
    InitialStateLoaded {
        auto_check: bool,
        auto_install: bool,
        last_check: Option<chrono::DateTime<chrono::Utc>>,
    },
}

impl Default for State {
    fn default() -> Self {
        Self {
            current_version: crate::update::current_version().to_string(),
            update_info: Resource::NotAsked,
            auto_check_enabled: false,
            auto_install_enabled: false,
            last_check_time: None,
            installing: false,
        }
    }
}

impl crate::tui::AppState for State {}

impl App for UpdateApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: ()) -> (State, Command<Msg>) {
        let state = State::default();

        // Load initial state (auto-check, auto-install settings and last check time)
        let cmd = Command::perform(
            async {
                let config = crate::global_config();
                let options = crate::config::options::Options::new(
                    config.pool.clone(),
                    crate::options_registry(),
                );

                // Load auto-check setting
                let auto_check = options.get_bool("update.auto_check").await.unwrap_or(false);

                // Load auto-install setting
                let auto_install = options
                    .get_bool("update.auto_install")
                    .await
                    .unwrap_or(false);

                // Load last check time
                let last_check =
                    crate::config::repository::update_metadata::get_last_check_time(&config.pool)
                        .await
                        .ok()
                        .flatten();

                (auto_check, auto_install, last_check)
            },
            |(auto_check, auto_install, last_check)| Msg::InitialStateLoaded {
                auto_check,
                auto_install,
                last_check,
            },
        );

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::InitialStateLoaded {
                auto_check,
                auto_install,
                last_check,
            } => {
                state.auto_check_enabled = auto_check;
                state.auto_install_enabled = auto_install;
                state.last_check_time = last_check;
                Command::None
            }

            Msg::CheckForUpdates => {
                state.update_info = Resource::Loading;
                Command::perform(
                    async {
                        crate::update::check_for_updates()
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Msg::UpdateInfoLoaded,
                )
            }

            Msg::UpdateInfoLoaded(result) => {
                match result {
                    Ok(info) => {
                        let needs_update = info.needs_update;
                        state.update_info = Resource::Success(info);
                        state.last_check_time = Some(chrono::Utc::now());

                        let mut commands = vec![
                            // Update last check timestamp in database
                            Command::perform(
                                async {
                                    let config = crate::global_config();
                                    let now = chrono::Utc::now();
                                    crate::config::repository::update_metadata::set_last_check_time(
                                        &config.pool,
                                        now,
                                    )
                                    .await
                                    .map_err(|e| e.to_string())
                                },
                                |_result| Msg::TimerTick, // Dummy message, we don't need to handle the result
                            ),
                        ];

                        // Auto-install if enabled and update is available
                        if state.auto_install_enabled && needs_update {
                            state.installing = true;
                            commands.push(Command::perform(
                                async {
                                    crate::update::install_update(false)
                                        .await
                                        .map_err(|e| e.to_string())
                                },
                                Msg::UpdateInstalled,
                            ));
                        }

                        Command::batch(commands)
                    }
                    Err(e) => {
                        state.update_info = Resource::Failure(e);
                        Command::None
                    }
                }
            }

            Msg::InstallUpdate => {
                state.installing = true;
                Command::perform(
                    async {
                        crate::update::install_update(false)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Msg::UpdateInstalled,
                )
            }

            Msg::UpdateInstalled(result) => {
                state.installing = false;
                match result {
                    Ok(version) => {
                        // Refresh update info to show we're up to date
                        Command::perform(
                            async {
                                crate::update::check_for_updates()
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            Msg::UpdateInfoLoaded,
                        )
                    }
                    Err(e) => {
                        state.update_info =
                            Resource::Failure(format!("Installation failed: {}", e));
                        Command::None
                    }
                }
            }

            Msg::TimerTick => {
                // Only check if auto-check is enabled
                if !state.auto_check_enabled {
                    return Command::None;
                }

                state.update_info = Resource::Loading;
                Command::perform(
                    async {
                        crate::update::check_for_updates()
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Msg::UpdateInfoLoaded,
                )
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        use crate::tui::LayoutConstraint::*;

        // Version info panel
        let mut version_elements = vec![Element::text(Line::from(vec![
            Span::styled(
                "Current Version: ",
                Style::default().fg(theme.text_secondary),
            ),
            Span::styled(
                &state.current_version,
                Style::default().fg(theme.accent_primary).bold(),
            ),
        ]))];

        // Show update status
        match &state.update_info {
            Resource::NotAsked => {
                version_elements.push(Element::text(Line::from(vec![
                    Span::styled(
                        "Latest Version:  ",
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::styled("Not checked", Style::default().fg(theme.text_tertiary)),
                ])));
            }
            Resource::Loading => {
                version_elements.push(Element::text(Line::from(vec![
                    Span::styled(
                        "Latest Version:  ",
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::styled("Checking...", Style::default().fg(theme.accent_tertiary)),
                ])));
            }
            Resource::Success(info) => {
                version_elements.push(Element::text(Line::from(vec![
                    Span::styled(
                        "Latest Version:  ",
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::styled(
                        &info.latest,
                        Style::default().fg(theme.accent_primary).bold(),
                    ),
                ])));

                version_elements.push(spacer!());

                if info.needs_update {
                    version_elements.push(Element::text(Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
                        Span::styled(
                            "Update available!",
                            Style::default().fg(theme.accent_success).bold(),
                        ),
                    ])));
                } else {
                    version_elements.push(Element::text(Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
                        Span::styled("Up to date", Style::default().fg(theme.accent_success)),
                    ])));
                }
            }
            Resource::Failure(err) => {
                version_elements.push(Element::text(Line::from(vec![
                    Span::styled("Error: ", Style::default().fg(theme.accent_error).bold()),
                    Span::styled(err, Style::default().fg(theme.text_primary)),
                ])));
            }
        }

        // Last check time
        if let Some(last_check) = state.last_check_time {
            version_elements.push(spacer!());
            let local_time = last_check.with_timezone(&chrono::Local);
            version_elements.push(Element::text(Line::from(vec![
                Span::styled("Last Check: ", Style::default().fg(theme.text_secondary)),
                Span::styled(
                    local_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Style::default().fg(theme.text_primary),
                ),
            ])));
        }

        let mut version_col = crate::tui::element::ColumnBuilder::new();
        for element in version_elements {
            version_col = version_col.add(element, Length(1));
        }
        let version_panel = Element::panel(version_col.build())
            .title("Version Information")
            .build();

        // Actions panel
        let mut action_elements = vec![];

        // Check for updates button
        if matches!(state.update_info, Resource::Loading) {
            action_elements.push(Element::text(Line::from(vec![Span::styled(
                "⏳ Checking for updates...",
                Style::default().fg(theme.accent_tertiary),
            )])));
        } else {
            action_elements.push(
                Element::button(FocusId::new("check-btn"), "Check for Updates")
                    .on_press(Msg::CheckForUpdates)
                    .build(),
            );
        }

        action_elements.push(spacer!());

        // Install button (if update available)
        if let Resource::Success(info) = &state.update_info {
            if info.needs_update {
                if state.installing {
                    action_elements.push(Element::text(Line::from(vec![Span::styled(
                        "⏳ Installing update...",
                        Style::default().fg(theme.accent_tertiary),
                    )])));
                } else {
                    action_elements.push(
                        Element::button(FocusId::new("install-btn"), "Install Update")
                            .on_press(Msg::InstallUpdate)
                            .build(),
                    );
                }
            }
        }

        let mut actions_col = crate::tui::element::ColumnBuilder::new();
        for element in action_elements {
            actions_col = actions_col.add(element, Length(3));
        }
        let actions_panel = Element::panel(actions_col.build()).title("Actions").build();

        // Main layout
        let content = col![
            version_panel => Length(8),
            actions_panel => Fill(1),
        ];

        let main_panel = Element::panel(content).title("Software Updates").build();

        LayeredView::new(main_panel)
    }

    fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
        let mut subs = vec![];

        // Add timer if auto-check is enabled
        if state.auto_check_enabled {
            subs.push(Subscription::timer(
                Duration::from_secs(3600), // 1 hour
                Msg::TimerTick,
            ));
        }

        subs
    }

    fn title() -> &'static str {
        "Updates"
    }

    fn status(_state: &State) -> Option<Line<'static>> {
        None
    }
}

pub struct UpdateApp;
