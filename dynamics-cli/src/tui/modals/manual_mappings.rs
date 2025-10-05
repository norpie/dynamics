//! Manual mappings modal for viewing and deleting field mappings

use crate::tui::{Element, Theme, FocusId};
use crate::tui::element::{LayoutConstraint, RowBuilder, ColumnBuilder};
use crate::tui::widgets::{ListState, ListItem};
use crate::{button_row, col, spacer, use_constraints};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Manual mapping for display in the list
#[derive(Clone)]
pub struct ManualMappingItem<Msg> {
    pub source_field: String,
    pub target_field: String,
    pub on_delete: Msg,
}

impl<Msg: Clone> ListItem for ManualMappingItem<Msg> {
    type Msg = Msg;

    fn to_element(&self, theme: &Theme, is_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let display = format!("{} → {}", self.source_field, self.target_field);

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(display, Style::default().fg(theme.text))
        ]));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.surface0));
        }

        builder.build()
    }
}

/// Builder for manual mappings management modal
///
/// # Example
/// ```rust
/// let modal = ManualMappingsModal::new()
///     .mappings(manual_mappings)
///     .list_state(list_state)
///     .on_delete(Msg::DeleteManualMapping)
///     .on_close(Msg::CloseManualMappingsModal)
///     .build(theme);
/// ```
pub struct ManualMappingsModal<Msg> {
    mappings: Vec<ManualMappingItem<Msg>>,
    list_state: ListState,
    on_list_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
    on_delete: Option<Msg>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> ManualMappingsModal<Msg> {
    /// Create a new manual mappings modal
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
            list_state: ListState::new(),
            on_list_navigate: None,
            on_delete: None,
            on_close: None,
            width: Some(70),
            height: Some(25),
        }
    }

    /// Set the list of manual mappings
    pub fn mappings(mut self, mappings: Vec<ManualMappingItem<Msg>>) -> Self {
        self.mappings = mappings;
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
    pub fn build(self, theme: &Theme) -> Element<Msg> {
        use_constraints!();

        // Build list
        let list_handler = self.on_list_navigate
            .expect("ManualMappingsModal requires on_list_navigate");
        let mappings_list = Element::list(
            FocusId::new("manual-mappings-list"),
            &self.mappings,
            &self.list_state,
            theme,
        )
        .on_navigate(list_handler)
        .build();

        let mappings_panel = Element::panel(mappings_list)
            .title("Manual Field Mappings")
            .build();

        // Buttons
        let buttons = button_row![
            ("manual-delete", "Delete (d)", self.on_delete.clone().expect("ManualMappingsModal requires on_delete")),
            ("manual-close", "Close (Esc)", self.on_close.clone().expect("ManualMappingsModal requires on_close")),
        ];

        // Layout with explicit constraints
        let modal_body = col![
            Element::styled_text(
                Line::from(vec![
                    Span::styled("Manual Mappings", Style::default().fg(theme.mauve).bold())
                ])
            ).build() => Length(1),
            spacer!() => Length(1),
            mappings_panel => Fill(1),
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

impl<Msg: Clone> Default for ManualMappingsModal<Msg> {
    fn default() -> Self {
        Self::new()
    }
}
