//! Prefix mappings modal for managing source-to-target prefix transformations

use crate::tui::{Element, Theme, FocusId};
use crate::tui::element::{LayoutConstraint, RowBuilder, ColumnBuilder};
use crate::tui::widgets::{ListState, ListItem, TextInputField};
use crate::{button_row, col, spacer, use_constraints};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Prefix mapping for display in the list
#[derive(Clone)]
pub struct PrefixMappingItem<Msg> {
    pub source_prefix: String,
    pub target_prefix: String,
    pub on_delete: Msg,
}

impl<Msg: Clone> ListItem for PrefixMappingItem<Msg> {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let display = format!("{} → {}", self.source_prefix, self.target_prefix);

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(display, Style::default().fg(theme.text_primary))
        ]));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Builder for prefix mappings management modal
///
/// # Example
/// ```rust
/// let modal = PrefixMappingsModal::new()
///     .mappings(prefix_mappings)
///     .source_input_field(source_field)
///     .target_input_field(target_field)
///     .list_state(list_state)
///     .on_add(Msg::AddPrefixMapping)
///     .on_delete(Msg::DeletePrefixMapping)
///     .on_close(Msg::ClosePrefixMappingsModal)
///     .build(theme);
/// ```
pub struct PrefixMappingsModal<Msg> {
    mappings: Vec<PrefixMappingItem<Msg>>,
    source_input_state: TextInputField,
    target_input_state: TextInputField,
    list_state: ListState,
    on_source_input_event: Option<fn(crate::tui::widgets::TextInputEvent) -> Msg>,
    on_target_input_event: Option<fn(crate::tui::widgets::TextInputEvent) -> Msg>,
    on_list_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
    on_list_select: Option<fn(usize) -> Msg>,
    on_add: Option<Msg>,
    on_delete: Option<Msg>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> PrefixMappingsModal<Msg> {
    /// Create a new prefix mappings modal
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
            source_input_state: TextInputField::new(),
            target_input_state: TextInputField::new(),
            list_state: ListState::new(),
            on_source_input_event: None,
            on_target_input_event: None,
            on_list_navigate: None,
            on_list_select: None,
            on_add: None,
            on_delete: None,
            on_close: None,
            width: Some(70),
            height: Some(25),
        }
    }

    /// Set the list of prefix mappings
    pub fn mappings(mut self, mappings: Vec<PrefixMappingItem<Msg>>) -> Self {
        self.mappings = mappings;
        self
    }

    /// Set the source input state
    pub fn source_input_state(mut self, state: TextInputField) -> Self {
        self.source_input_state = state;
        self
    }

    /// Set the target input state
    pub fn target_input_state(mut self, state: TextInputField) -> Self {
        self.target_input_state = state;
        self
    }

    /// Set the list state
    pub fn list_state(mut self, state: ListState) -> Self {
        self.list_state = state;
        self
    }

    /// Set source input event handler
    pub fn on_source_input_event(mut self, handler: fn(crate::tui::widgets::TextInputEvent) -> Msg) -> Self {
        self.on_source_input_event = Some(handler);
        self
    }

    /// Set target input event handler
    pub fn on_target_input_event(mut self, handler: fn(crate::tui::widgets::TextInputEvent) -> Msg) -> Self {
        self.on_target_input_event = Some(handler);
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

    /// Set the message sent when Add is clicked
    pub fn on_add(mut self, msg: Msg) -> Self {
        self.on_add = Some(msg);
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

        // Build text input elements from state
        let source_handler = self.on_source_input_event
            .expect("PrefixMappingsModal requires on_source_input_event");
        let source_input = Element::text_input(
            FocusId::new("prefix-source-input"),
            &self.source_input_state.value,
            &self.source_input_state.state,
        )
        .placeholder("e.g., cr123_")
        .on_event(source_handler)
        .build();

        let target_handler = self.on_target_input_event
            .expect("PrefixMappingsModal requires on_target_input_event");
        let target_input = Element::text_input(
            FocusId::new("prefix-target-input"),
            &self.target_input_state.value,
            &self.target_input_state.state,
        )
        .placeholder("e.g., new_")
        .on_event(target_handler)
        .build();

        // Build list
        let list_handler = self.on_list_navigate
            .expect("PrefixMappingsModal requires on_list_navigate");
        let select_handler = self.on_list_select
            .expect("PrefixMappingsModal requires on_list_select");
        let mappings_list = Element::list(
            FocusId::new("prefix-list"),
            &self.mappings,
            &self.list_state,
            theme,
        )
        .on_select(select_handler)
        .on_navigate(list_handler)
        .build();

        // Input panels
        let source_panel = Element::panel(source_input)
            .title("Source Prefix")
            .build();

        let target_panel = Element::panel(target_input)
            .title("Target Prefix")
            .build();

        let mappings_panel = Element::panel(mappings_list)
            .title("Saved Prefix Mappings")
            .build();

        // Buttons
        let buttons = button_row![
            ("prefix-add", "Add", self.on_add.clone().expect("PrefixMappingsModal requires on_add")),
            ("prefix-delete", "Delete", self.on_delete.clone().expect("PrefixMappingsModal requires on_delete")),
            ("prefix-close", "Close", self.on_close.clone().expect("PrefixMappingsModal requires on_close")),
        ];

        // Layout with explicit constraints
        let modal_body = col![
            Element::styled_text(
                Line::from(vec![
                    Span::styled("Prefix Mappings", Style::default().fg(theme.accent_tertiary).bold())
                ])
            ).build() => Length(1),
            spacer!() => Length(1),
            source_panel => Length(3),
            spacer!() => Length(1),
            target_panel => Length(3),
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

impl<Msg: Clone> Default for PrefixMappingsModal<Msg> {
    fn default() -> Self {
        Self::new()
    }
}
