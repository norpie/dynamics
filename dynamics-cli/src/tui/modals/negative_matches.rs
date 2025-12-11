//! Negative matches modal for managing fields blocked from prefix matching

use crate::tui::{Element, Theme, FocusId};
use crate::tui::element::{LayoutConstraint, RowBuilder, ColumnBuilder};
use crate::tui::widgets::{ListState, ListItem};
use crate::{button_row, col, spacer, use_constraints};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Negative match item for display in the list
#[derive(Clone)]
pub struct NegativeMatchItem<Msg> {
    pub source_field: String,
    pub on_delete: Msg,
}

impl<Msg: Clone> ListItem for NegativeMatchItem<Msg> {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let display_text = format!("⛔ {}", self.source_field);
        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(display_text, Style::default().fg(theme.text_primary))
        ]));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Builder for negative matches management modal
///
/// # Example
/// ```rust
/// let modal = NegativeMatchesModal::new()
///     .matches(negative_match_items)
///     .list_state(list_state)
///     .on_delete(Msg::DeleteNegativeMatch)
///     .on_close(Msg::CloseNegativeMatchesModal)
///     .build();
/// ```
pub struct NegativeMatchesModal<Msg> {
    matches: Vec<NegativeMatchItem<Msg>>,
    list_state: ListState,
    on_list_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
    on_list_select: Option<fn(usize) -> Msg>,
    on_delete: Option<Msg>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> NegativeMatchesModal<Msg> {
    /// Create a new negative matches modal
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            list_state: ListState::new(),
            on_list_navigate: None,
            on_list_select: None,
            on_delete: None,
            on_close: None,
            width: Some(70),
            height: Some(25),
        }
    }

    /// Set the list of negative matches
    pub fn matches(mut self, matches: Vec<NegativeMatchItem<Msg>>) -> Self {
        self.matches = matches;
        self
    }

    /// Set the list state
    pub fn list_state(mut self, state: ListState) -> Self {
        self.list_state = state;
        self
    }

    /// Set list navigation handler
    pub fn on_list_navigate(mut self, handler: fn(crossterm::event::KeyCode) -> Msg) -> Self {
        self.on_list_navigate = Some(handler);
        self
    }

    /// Set list select handler
    pub fn on_list_select(mut self, handler: fn(usize) -> Msg) -> Self {
        self.on_list_select = Some(handler);
        self
    }

    /// Set the message sent when Delete is clicked
    pub fn on_delete(mut self, msg: Msg) -> Self {
        self.on_delete = Some(msg);
        self
    }

    /// Set the message sent when Close is clicked
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Set modal width
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set modal height
    pub fn height(mut self, height: u16) -> Self {
        self.height = Some(height);
        self
    }

    /// Build the modal Element
    pub fn build(self) -> Element<Msg> {
        use_constraints!();
        let theme = &crate::global_runtime_config().theme;

        // Build list
        let list_handler = self.on_list_navigate
            .expect("NegativeMatchesModal requires on_list_navigate");
        let select_handler = self.on_list_select
            .expect("NegativeMatchesModal requires on_list_select");
        let matches_list = Element::list(
            FocusId::new("negative-matches-list"),
            &self.matches,
            &self.list_state,
            theme,
        )
        .on_select(select_handler)
        .on_navigate(list_handler)
        .build();

        let matches_panel = Element::panel(matches_list)
            .title("Blocked Fields (Prefix Match Override)")
            .build();

        // Buttons
        let buttons = button_row![
            ("negative-delete", "Delete", self.on_delete.clone().expect("NegativeMatchesModal requires on_delete")),
            ("negative-close", "Close", self.on_close.clone().expect("NegativeMatchesModal requires on_close")),
        ];

        // Help text
        let help_text = Element::styled_text(
            Line::from(vec![
                Span::styled("These fields are blocked from automatic prefix matching. ", Style::default().fg(theme.text_secondary)),
                Span::styled("Press ", Style::default().fg(theme.text_secondary)),
                Span::styled("d", Style::default().fg(theme.accent_tertiary).bold()),
                Span::styled(" on a prefix-matched field to add it.", Style::default().fg(theme.text_secondary)),
            ])
        ).build();

        // Layout with explicit constraints
        let modal_body = col![
            Element::styled_text(
                Line::from(vec![
                    Span::styled("Negative Matches", Style::default().fg(theme.accent_tertiary).bold())
                ])
            ).build() => Length(1),
            spacer!() => Length(1),
            help_text => Length(2),
            spacer!() => Length(1),
            matches_panel => Fill(1),
            spacer!() => Length(1),
            buttons => Length(3),
        ];

        // Wrap in outer panel with title, width, and height
        Element::panel(
            Element::container(modal_body)
                .padding(2)
                .build()
        )
        .width(self.width.unwrap_or(70))
        .height(self.height.unwrap_or(25))
        .build()
    }
}

impl<Msg: Clone> Default for NegativeMatchesModal<Msg> {
    fn default() -> Self {
        Self::new()
    }
}
