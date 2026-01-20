use crate::tui::element::{ColumnBuilder, LayoutConstraint};
use crate::tui::widgets::ScrollableState;
use crate::tui::{Element, FocusId, Theme};
use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use std::collections::HashMap;

/// Helper to format a KeyCode for display (e.g., Char('i') → "i", F(1) → "F1")
fn format_key(key: &KeyCode) -> String {
    match key {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{}", n),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Insert => "Ins".to_string(),
        _ => format!("{:?}", key),
    }
}

/// Group keybindings by description and format as aliases (e.g., "n/N")
fn group_and_format_bindings(bindings: &[(KeyCode, String)]) -> Vec<(String, String)> {
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();

    for (key, desc) in bindings {
        grouped
            .entry(desc.clone())
            .or_default()
            .push(format_key(key));
    }

    let mut result: Vec<(String, String)> = grouped
        .into_iter()
        .map(|(desc, mut keys)| {
            keys.sort(); // Consistent ordering
            let key_str = keys.join("/");
            (key_str, desc)
        })
        .collect();

    // Sort by key string for consistent display
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Builder for help menu modal with scrollable keybinding display
///
/// # Example
/// ```rust
/// let modal = HelpModal::new()
///     .global_bindings(vec![(KeyCode::F(1), "Toggle help".to_string())])
///     .current_app("Migration Environment", vec![(KeyCode::Char('n'), "New migration".to_string())])
///     .add_app("App Launcher", vec![(KeyCode::Enter, "Launch app".to_string())])
///     .scroll_state(&mut scroll_state)
///     .build(theme);
/// ```
pub struct HelpModal<'a> {
    global_bindings: Vec<(KeyCode, String)>,
    current_app_title: Option<String>,
    current_app_bindings: Vec<(KeyCode, String)>,
    other_apps: Vec<(String, Vec<(KeyCode, String)>)>,
    scroll_state: Option<&'a ScrollableState>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<'a> HelpModal<'a> {
    /// Create a new help modal builder
    pub fn new() -> Self {
        Self {
            global_bindings: Vec::new(),
            current_app_title: None,
            current_app_bindings: Vec::new(),
            other_apps: Vec::new(),
            scroll_state: None,
            width: Some(60),
            height: Some(20),
        }
    }

    /// Set global keybindings
    pub fn global_bindings(mut self, bindings: Vec<(KeyCode, String)>) -> Self {
        self.global_bindings = bindings;
        self
    }

    /// Set current app keybindings
    pub fn current_app(
        mut self,
        title: impl Into<String>,
        bindings: Vec<(KeyCode, String)>,
    ) -> Self {
        self.current_app_title = Some(title.into());
        self.current_app_bindings = bindings;
        self
    }

    /// Add another app's keybindings
    pub fn add_app(mut self, title: impl Into<String>, bindings: Vec<(KeyCode, String)>) -> Self {
        self.other_apps.push((title.into(), bindings));
        self
    }

    /// Set the scroll state reference (required for scrolling)
    pub fn scroll_state(mut self, state: &'a ScrollableState) -> Self {
        self.scroll_state = Some(state);
        self
    }

    /// Set modal width (default: 60)
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set modal height (default: 20)
    pub fn height(mut self, height: u16) -> Self {
        self.height = Some(height);
        self
    }

    /// Build the modal Element (with unit type since it's global UI)
    pub fn build(self) -> Element<()> {
        // Build ALL help content items (no skipping - scrollable widget handles scrolling)
        let theme = &crate::global_runtime_config().theme;
        let mut help_items = vec![
            Element::styled_text(Line::from(vec![Span::styled(
                "Keyboard Shortcuts",
                Style::default().fg(theme.accent_primary).bold(),
            )]))
            .build(),
            Element::text(""),
        ];

        // Section 1: Global Keys (highest priority)
        if !self.global_bindings.is_empty() {
            help_items.push(
                Element::styled_text(Line::from(vec![Span::styled(
                    "▼ Global",
                    Style::default().fg(theme.accent_muted).bold(),
                )]))
                .build(),
            );

            let formatted_global = group_and_format_bindings(&self.global_bindings);
            for (key_str, description) in &formatted_global {
                let line = Line::from(vec![
                    Span::styled(
                        format!("  {:13}", key_str),
                        Style::default().fg(theme.accent_tertiary),
                    ),
                    Span::raw("  "),
                    Span::styled(description.clone(), Style::default().fg(theme.text_primary)),
                ]);
                help_items.push(Element::styled_text(line).build());
            }

            help_items.push(Element::text(""));
        }

        // Section 2: Current App Keys
        if let Some(title) = self.current_app_title {
            help_items.push(
                Element::styled_text(Line::from(vec![Span::styled(
                    format!("▼ {}", title),
                    Style::default().fg(theme.accent_secondary).bold(),
                )]))
                .build(),
            );

            let formatted_current = group_and_format_bindings(&self.current_app_bindings);
            for (key_str, description) in &formatted_current {
                let line = Line::from(vec![
                    Span::styled(
                        format!("  {:13}", key_str),
                        Style::default().fg(theme.accent_success),
                    ),
                    Span::raw("  "),
                    Span::styled(description.clone(), Style::default().fg(theme.text_primary)),
                ]);
                help_items.push(Element::styled_text(line).build());
            }

            help_items.push(Element::text(""));
        }

        // Section 3: Other Apps
        for (app_title, app_bindings) in &self.other_apps {
            help_items.push(
                Element::styled_text(Line::from(vec![Span::styled(
                    format!("▼ {}", app_title),
                    Style::default().fg(theme.border_primary).bold(),
                )]))
                .build(),
            );

            let formatted_other = group_and_format_bindings(app_bindings);
            for (key_str, description) in &formatted_other {
                let line = Line::from(vec![
                    Span::styled(
                        format!("  {:13}", key_str),
                        Style::default().fg(theme.border_tertiary),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        description.clone(),
                        Style::default().fg(theme.text_tertiary),
                    ),
                ]);
                help_items.push(Element::styled_text(line).build());
            }

            help_items.push(Element::text(""));
        }

        help_items.push(Element::text(""));
        help_items.push(
            Element::styled_text(Line::from(vec![Span::styled(
                "[ESC to close | ↑↓/PgUp/PgDn/Home/End to scroll]",
                Style::default().fg(theme.border_primary),
            )]))
            .build(),
        );

        // Create scrollable column with all items (spacing=0 for dense packing)
        let mut column_builder = ColumnBuilder::new();
        for item in help_items {
            column_builder = column_builder.add(item, LayoutConstraint::Length(1));
        }
        let help_column = column_builder.spacing(0).build();

        // Wrap in scrollable if we have scroll state
        let content = if let Some(scroll_state) = self.scroll_state {
            Element::scrollable(FocusId::new("help_scroll"), help_column, scroll_state).build()
        } else {
            help_column
        };

        // Wrap in container with padding
        let padded_content = Element::container(content).padding(1).build();

        // Wrap in panel
        let mut panel = Element::panel(padded_content).title("Help");

        if let Some(w) = self.width {
            panel = panel.width(w);
        }
        if let Some(h) = self.height {
            panel = panel.height(h);
        }

        panel.build()
    }
}
