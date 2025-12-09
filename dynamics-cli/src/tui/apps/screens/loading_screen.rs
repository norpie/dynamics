use crossterm::event::KeyCode;
use ratatui::text::{Line, Span};
use ratatui::style::Style;
use ratatui::prelude::Stylize;
use serde_json::Value;
use crate::tui::{App, AppId, Command, Element, Subscription, Theme, LayoutConstraint, LayeredView};
use crate::tui::element::ColumnBuilder;

pub struct LoadingScreen;

pub struct LoadingScreenParams {
    pub tasks: Vec<String>,
    pub target: Option<AppId>,
    pub caller: Option<AppId>,
    pub cancellable: bool,
}

impl Default for LoadingScreenParams {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            target: None,
            caller: None,
            cancellable: false,
        }
    }
}

#[derive(Clone)]
pub enum Msg {
    TaskProgress(Value),
    Tick,
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct LoadingTask {
    pub name: String,
    pub status: TaskStatus,
    /// Optional progress text (e.g., "500 records fetched")
    pub progress: Option<String>,
}

#[derive(Default)]
pub struct State {
    tasks: Vec<LoadingTask>,
    target_app: Option<AppId>,
    caller_app: Option<AppId>,
    cancellable: bool,
    spinner_state: usize,
    countdown_ticks: Option<usize>, // Number of ticks remaining before navigation (80ms per tick)
    navigation_sent: bool, // Prevent sending navigation multiple times
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl crate::tui::AppState for State {}

impl App for LoadingScreen {
    type State = State;
    type Msg = Msg;
    type InitParams = LoadingScreenParams;

    fn init(params: LoadingScreenParams) -> (State, Command<Msg>) {
        let mut state = State::default();

        state.tasks = params.tasks
            .iter()
            .map(|name| LoadingTask {
                name: name.clone(),
                status: TaskStatus::Pending,
                progress: None,
            })
            .collect();

        state.target_app = params.target;
        state.caller_app = params.caller;
        state.cancellable = params.cancellable;

        (state, Command::None)
    }

    fn quit_policy() -> crate::tui::QuitPolicy {
        crate::tui::QuitPolicy::Sleep
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::TaskProgress(data) => {
                log::info!("✓ LoadingScreen::TaskProgress - received loading:progress event");
                log::debug!("  Event data: {:?}", data);

                let task_name = data.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let status_str = data.get("status").and_then(|v| v.as_str()).unwrap_or("");

                log::info!("  Task: '{}', Status: '{}'", task_name, status_str);

                if let Some(task) = state.tasks.iter_mut().find(|t| t.name == task_name) {
                    // Don't downgrade from Completed/Failed back to InProgress
                    let already_done = matches!(task.status, TaskStatus::Completed | TaskStatus::Failed(_));

                    if !already_done || status_str == "Completed" || status_str == "Failed" {
                        task.status = match status_str {
                            "InProgress" => TaskStatus::InProgress,
                            "Completed" => {
                                task.progress = None; // Clear progress on completion
                                TaskStatus::Completed
                            }
                            "Failed" => TaskStatus::Failed(
                                data.get("error")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Unknown error")
                                    .to_string()
                            ),
                            _ => TaskStatus::Pending,
                        };
                    }

                    // Update progress text if provided (but not if already completed)
                    if !already_done {
                        if let Some(progress) = data.get("progress").and_then(|v| v.as_str()) {
                            task.progress = Some(progress.to_string());
                        }
                    }
                }

                // Check if all tasks are complete
                // IMPORTANT: Only start countdown if we have tasks AND they're all done
                // This prevents edge cases where empty task list triggers immediate countdown
                let all_done = !state.tasks.is_empty() && state.tasks.iter().all(|t| {
                    matches!(t.status, TaskStatus::Completed | TaskStatus::Failed(_))
                });

                if all_done && state.countdown_ticks.is_none() {
                    log::info!("✓ LoadingScreen - all tasks complete, starting countdown");
                    // Start countdown: 80ms per tick
                    state.countdown_ticks = Some(1);
                }

                Command::None
            }

            Msg::Tick => {
                state.spinner_state = (state.spinner_state + 1) % SPINNER_FRAMES.len();

                if let Some(remaining) = state.countdown_ticks {
                    if remaining <= 1 {
                        // Only navigate if we haven't already sent navigation
                        if !state.navigation_sent {
                            // Only navigate if we have tasks (prevents stale countdown from navigating)
                            if !state.tasks.is_empty() {
                                if let Some(target) = state.target_app {
                                    log::info!("✓ LoadingScreen - countdown complete, navigating to {:?}", target);
                                    state.navigation_sent = true; // Mark navigation as sent
                                    return Command::batch(vec![
                                        Command::navigate_to(target),
                                        Command::quit_self(), // Clean up after navigation
                                    ]);
                                } else {
                                    log::warn!("✗ LoadingScreen - countdown complete but target_app is None!");
                                }
                            } else {
                                log::warn!("✗ LoadingScreen - countdown complete but tasks is empty!");
                            }
                        } else {
                            log::debug!("⏸️  LoadingScreen - countdown complete but navigation already sent, skipping");
                        }
                    } else {
                        state.countdown_ticks = Some(remaining - 1);
                    }
                }

                Command::None
            }

            Msg::Cancel => {
                if let Some(caller) = state.caller_app {
                    // Notify caller to cancel work
                    let topic = format!("loading:cancel:{:?}", caller);
                    Command::Batch(vec![
                        Command::Publish {
                            topic,
                            data: serde_json::json!({}),
                        },
                        Command::navigate_to(caller),
                    ])
                } else {
                    Command::None
                }
            }

        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let mut content = vec![];
        let theme = &crate::global_runtime_config().theme;

        // Header
        let all_done = state.tasks.iter().all(|t| {
            matches!(t.status, TaskStatus::Completed | TaskStatus::Failed(_))
        });

        let header_text = if state.countdown_ticks.is_some() {
            "All tasks completed! Returning in 1 second...".to_string()
        } else if all_done {
            "All tasks completed!".to_string()
        } else {
            "Loading...".to_string()
        };

        content.push(Element::styled_text(Line::from(vec![
            Span::styled(SPINNER_FRAMES[state.spinner_state], Style::default().fg(theme.palette_4).bold()),
            Span::raw(" "),
            Span::styled(header_text.clone(), Style::default().fg(theme.palette_4)),
        ])).build());
        content.push(Element::text(""));

        // Tasks
        for task in &state.tasks {
            let (symbol, color) = match &task.status {
                TaskStatus::Pending => ("◯", theme.border_primary),
                TaskStatus::InProgress => (SPINNER_FRAMES[state.spinner_state], theme.palette_4),
                TaskStatus::Completed => ("✓", theme.accent_success),
                TaskStatus::Failed(_) => ("❌", theme.accent_error),
            };

            let mut spans = vec![
                Span::styled(format!(" {} ", symbol), Style::default().fg(color)),
                Span::styled(task.name.clone(), Style::default().fg(color)),
            ];

            // Add progress text if available
            if let Some(ref progress) = task.progress {
                spans.push(Span::styled(
                    format!(" ({})", progress),
                    Style::default().fg(theme.text_secondary),
                ));
            }

            content.push(Element::styled_text(Line::from(spans)).build());
        }

        content.push(Element::text(""));

        // Footer
        let footer_text = if all_done {
            "Press any key to continue..."
        } else if state.cancellable {
            "Press ESC to cancel..."
        } else {
            "Please wait..."
        };

        content.push(Element::styled_text(Line::from(
            Span::styled(footer_text, Style::default().fg(theme.border_primary))
        )).build());

        // Wrap in panel
        let panel = Element::panel(
            Element::container(
                ColumnBuilder::new()
                    .add(Element::column(content).build(), LayoutConstraint::Fill(1))
                    .build()
            )
            .padding(2)
            .build()
        )
        .title("Loading Tasks")
        .build();

        LayeredView::new(panel)
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        vec![
            Subscription::subscribe("loading:progress", |data| Some(Msg::TaskProgress(data))),
            Subscription::timer(std::time::Duration::from_millis(80), Msg::Tick),
            Subscription::keyboard(KeyCode::Esc, "Cancel loading", Msg::Cancel),
        ]
    }

    fn title() -> &'static str {
        "Loading"
    }

    fn suspend_policy() -> crate::tui::SuspendPolicy {
        crate::tui::SuspendPolicy::AlwaysActive
    }

    fn on_suspend(state: &mut State) -> Command<Msg> {
        log::info!("🛑 LoadingScreen::on_suspend - clearing countdown and navigation state");
        log::debug!("  Previous state: countdown_ticks={:?}, target_app={:?}, navigation_sent={}",
                    state.countdown_ticks, state.target_app, state.navigation_sent);

        // Clear countdown to prevent navigation while suspended
        state.countdown_ticks = None;

        // Clear target to prevent stale navigation
        state.target_app = None;

        // Reset navigation sent flag
        state.navigation_sent = false;

        Command::None
    }

    fn on_resume(state: &mut State) -> Command<Msg> {
        log::info!("▶️  LoadingScreen::on_resume - resuming loading screen");
        log::debug!("  Current state: {} tasks, countdown_ticks={:?}", state.tasks.len(), state.countdown_ticks);
        Command::None
    }
}
