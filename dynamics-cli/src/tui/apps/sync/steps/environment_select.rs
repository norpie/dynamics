//! Step 1: Environment Selection View
//!
//! Dual list selection for choosing origin and target environments.

use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};

use crate::config::models::DbEnvironment;
use crate::tui::element::Element;
use crate::tui::widgets::ListItem;
use crate::tui::state::theme::Theme;
use crate::tui::Resource;
use crate::{col, row, spacer, use_constraints};

use super::super::state::{State, EnvironmentSelectState};
use super::super::msg::Msg;

/// Environment list item for rendering
#[derive(Clone)]
struct EnvListItem {
    name: String,
    host: Option<String>,
    is_origin_selected: bool,
    is_target_selected: bool,
}

impl ListItem for EnvListItem {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        // Determine styling based on selection state
        let (icon, style) = if self.is_origin_selected && self.is_target_selected {
            // Shouldn't happen, but handle it
            ("⚠ ", Style::default().fg(theme.accent_warning))
        } else if self.is_origin_selected {
            ("◉ ", Style::default().fg(theme.accent_success).bold())
        } else if self.is_target_selected {
            ("◎ ", Style::default().fg(theme.accent_info).bold())
        } else {
            ("○ ", Style::default().fg(theme.text_secondary))
        };

        let bg_style = if is_selected {
            Style::default().bg(theme.bg_surface)
        } else {
            Style::default()
        };

        let host_text = self.host.as_ref()
            .map(|h| format!(" ({})", truncate_host(h, 40)))
            .unwrap_or_default();

        let text = format!("{}{}{}", icon, self.name, host_text);

        Element::styled_text(Line::from(Span::styled(text, style)))
            .background(bg_style)
            .build()
    }
}

/// Truncate host for display
fn truncate_host(host: &str, max_len: usize) -> String {
    if host.len() <= max_len {
        host.to_string()
    } else {
        format!("{}...", &host[..max_len - 3])
    }
}

/// Render the environment selection step
pub fn render_environment_select(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    // Build content based on loading state
    // We need to clone the error/envs to avoid borrow issues
    let content = match &state.env_select.environments {
        Resource::NotAsked | Resource::Loading => {
            render_loading(theme)
        }
        Resource::Failure(err) => {
            let err_clone = err.clone();
            render_error(&err_clone, theme)
        }
        Resource::Success(envs) => {
            let envs_clone = envs.clone();
            render_env_lists(state, &envs_clone, theme)
        }
    };

    // Wrap in step panel
    let header = render_step_header("Step 1: Select Environments", theme);
    let footer = render_step_footer(state, theme);

    col![
        header => Length(3),
        content => Fill(1),
        footer => Length(5),
    ]
}

/// Render loading state
fn render_loading(_theme: &Theme) -> Element<Msg> {
    Element::panel(Element::text("Loading environments..."))
        .title("Environments")
        .build()
}

/// Render error state
fn render_error(err: &str, _theme: &Theme) -> Element<Msg> {
    let text = format!("Error: {}", err);
    Element::panel(Element::text(text))
        .title("Error")
        .build()
}

/// Render the dual environment lists
fn render_env_lists(state: &mut State, envs: &[DbEnvironment], theme: &Theme) -> Element<Msg> {
    use_constraints!();

    // Build list items for origin
    let origin_items: Vec<EnvListItem> = envs.iter().map(|env| {
        EnvListItem {
            name: env.name.clone(),
            host: Some(env.host.clone()),
            is_origin_selected: state.env_select.origin_env.as_ref() == Some(&env.name),
            is_target_selected: state.env_select.target_env.as_ref() == Some(&env.name),
        }
    }).collect();

    // Build list items for target (same data, different selection highlight)
    let target_items = origin_items.clone();

    // Origin list
    let origin_title = if let Some(ref env) = state.env_select.origin_env {
        format!("Origin: {}", env)
    } else {
        "Origin (select one)".to_string()
    };

    let origin_list = Element::list(
        "origin-env-list",
        &origin_items,
        &state.env_select.origin_list,
        theme,
    )
    .on_select(Msg::OriginListSelect)
    .on_navigate(Msg::OriginListNavigate)
    .build();

    let origin_panel = Element::panel(origin_list)
        .title(origin_title)
        .build();

    // Target list
    let target_title = if let Some(ref env) = state.env_select.target_env {
        format!("Target: {}", env)
    } else {
        "Target (select one)".to_string()
    };

    let target_list = Element::list(
        "target-env-list",
        &target_items,
        &state.env_select.target_list,
        theme,
    )
    .on_select(Msg::TargetListSelect)
    .on_navigate(Msg::TargetListNavigate)
    .build();

    let target_panel = Element::panel(target_list)
        .title(target_title)
        .build();

    // Help text
    let help = Element::text("Tab Switch lists | Enter Select | ↑↓ Navigate");

    col![
        row![
            origin_panel => Fill(1),
            target_panel => Fill(1),
        ] => Fill(1),
        help => Length(1),
    ]
}

/// Render step header
fn render_step_header(title: &str, theme: &Theme) -> Element<Msg> {
    Element::styled_text(Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(theme.accent_primary).bold()
    ))).build()
}

/// Render step footer with navigation and validation
fn render_step_footer(state: &State, _theme: &Theme) -> Element<Msg> {
    use_constraints!();
    use crate::button_row;

    // Validation message
    let validation = if let Some(err) = state.env_select.validation_error() {
        let text = format!("⚠ {}", err);
        Element::text(text)
    } else {
        Element::text("✓ Ready to proceed")
    };

    // Navigation buttons
    let buttons = button_row![
        ("env-back-btn", "Back (Esc)", Msg::Back),
        ("env-next-btn", "Next (Enter)", Msg::Next),
    ];

    col![
        validation => Length(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ]
}
