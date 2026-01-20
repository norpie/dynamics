use crate::tui::element::ColumnBuilder;
use crate::tui::{
    App, AppId, Command, Element, LayeredView, LayoutConstraint, Subscription, Theme,
};
use crossterm::event::KeyCode;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

pub struct ErrorScreen;

pub struct ErrorScreenParams {
    pub message: String,
    pub target: Option<AppId>,
}

impl Default for ErrorScreenParams {
    fn default() -> Self {
        Self {
            message: "An error occurred".to_string(),
            target: Some(AppId::AppLauncher),
        }
    }
}

#[derive(Clone)]
pub enum Msg {
    Continue,
}

#[derive(Default)]
pub struct State {
    error_message: String,
    target_app: Option<AppId>,
}

impl crate::tui::AppState for State {}

impl App for ErrorScreen {
    type State = State;
    type Msg = Msg;
    type InitParams = ErrorScreenParams;

    fn init(params: ErrorScreenParams) -> (State, Command<Msg>) {
        let state = State {
            error_message: params.message,
            target_app: params.target,
        };

        (state, Command::None)
    }

    fn quit_policy() -> crate::tui::QuitPolicy {
        crate::tui::QuitPolicy::QuitOnExit
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Continue => {
                if let Some(target) = state.target_app {
                    Command::navigate_to(target)
                } else {
                    Command::None
                }
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let content = vec![
            Element::styled_text(Line::from(vec![Span::styled(
                "❌ Error",
                Style::default().fg(theme.accent_error).bold(),
            )]))
            .build(),
            Element::text(""),
            Element::text(&state.error_message),
            Element::text(""),
            Element::styled_text(Line::from(vec![Span::styled(
                "Press Enter to continue",
                Style::default().fg(theme.border_primary),
            )]))
            .build(),
        ];

        // Wrap in panel
        let panel = Element::panel(
            Element::container(
                ColumnBuilder::new()
                    .add(Element::column(content).build(), LayoutConstraint::Fill(1))
                    .build(),
            )
            .padding(2)
            .build(),
        )
        .title("Error")
        .build();

        LayeredView::new(panel)
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        vec![Subscription::keyboard(
            KeyCode::Enter,
            "Continue",
            Msg::Continue,
        )]
    }

    fn title() -> &'static str {
        "Error"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;
        Some(Line::from(Span::styled(
            "[Error]",
            Style::default().fg(theme.accent_error),
        )))
    }
}
