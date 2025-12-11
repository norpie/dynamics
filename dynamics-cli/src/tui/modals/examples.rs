//! Examples modal for managing example record pairs

use crate::tui::{Element, Theme, FocusId};
use crate::tui::element::{LayoutConstraint, RowBuilder, ColumnBuilder};
use crate::tui::widgets::{ListState, ListItem, TextInputField};
use crate::{button_row, col, spacer, use_constraints};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Example pair for display in the list
#[derive(Clone)]
pub struct ExamplePairItem<Msg> {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub label: Option<String>,
    pub on_delete: Msg,
}

impl<Msg: Clone> ListItem for ExamplePairItem<Msg> {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let display = if let Some(label) = &self.label {
            format!("{} ({}... → {}...)",
                label,
                &self.source_id[..8.min(self.source_id.len())],
                &self.target_id[..8.min(self.target_id.len())]
            )
        } else {
            format!("{}... → {}...",
                &self.source_id[..8.min(self.source_id.len())],
                &self.target_id[..8.min(self.target_id.len())]
            )
        };

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(display, Style::default().fg(theme.text_primary))
        ]));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Builder for examples management modal
///
/// # Example
/// ```rust
/// let modal = ExamplesModal::new()
///     .pairs(example_pairs)
///     .source_input_field(source_field)
///     .target_input_field(target_field)
///     .label_input_field(label_field)
///     .list_state(list_state)
///     .on_add(Msg::AddExamplePair)
///     .on_delete(Msg::DeleteExamplePair)
///     .on_close(Msg::CloseExamplesModal)
///     .build(theme);
/// ```
pub struct ExamplesModal<Msg> {
    pairs: Vec<ExamplePairItem<Msg>>,
    source_input_state: TextInputField,
    target_input_state: TextInputField,
    label_input_state: TextInputField,
    list_state: ListState,
    on_source_input_event: Option<fn(crate::tui::widgets::TextInputEvent) -> Msg>,
    on_target_input_event: Option<fn(crate::tui::widgets::TextInputEvent) -> Msg>,
    on_label_input_event: Option<fn(crate::tui::widgets::TextInputEvent) -> Msg>,
    on_list_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
    on_list_select: Option<fn(usize) -> Msg>,
    on_add: Option<Msg>,
    on_delete: Option<Msg>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> ExamplesModal<Msg> {
    /// Create a new examples modal
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            source_input_state: TextInputField::new(),
            target_input_state: TextInputField::new(),
            label_input_state: TextInputField::new(),
            list_state: ListState::new(),
            on_source_input_event: None,
            on_target_input_event: None,
            on_label_input_event: None,
            on_list_navigate: None,
            on_list_select: None,
            on_add: None,
            on_delete: None,
            on_close: None,
            width: Some(80),
            height: Some(30),
        }
    }

    /// Set the list of example pairs
    pub fn pairs(mut self, pairs: Vec<ExamplePairItem<Msg>>) -> Self {
        self.pairs = pairs;
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

    /// Set the label input state
    pub fn label_input_state(mut self, state: TextInputField) -> Self {
        self.label_input_state = state;
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

    /// Set label input event handler
    pub fn on_label_input_event(mut self, handler: fn(crate::tui::widgets::TextInputEvent) -> Msg) -> Self {
        self.on_label_input_event = Some(handler);
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
            .expect("ExamplesModal requires on_source_input_event");
        let source_input = Element::text_input(
            FocusId::new("examples-source-input"),
            &self.source_input_state.value,
            &self.source_input_state.state,
        )
        .placeholder("UUID")
        .on_event(source_handler)
        .build();

        let target_handler = self.on_target_input_event
            .expect("ExamplesModal requires on_target_input_event");
        let target_input = Element::text_input(
            FocusId::new("examples-target-input"),
            &self.target_input_state.value,
            &self.target_input_state.state,
        )
        .placeholder("UUID")
        .on_event(target_handler)
        .build();

        let label_handler = self.on_label_input_event
            .expect("ExamplesModal requires on_label_input_event");
        let label_input = Element::text_input(
            FocusId::new("examples-label-input"),
            &self.label_input_state.value,
            &self.label_input_state.state,
        )
        .placeholder("Optional")
        .on_event(label_handler)
        .build();

        // Build list
        let list_handler = self.on_list_navigate
            .expect("ExamplesModal requires on_list_navigate");
        let select_handler = self.on_list_select
            .expect("ExamplesModal requires on_list_select");
        let pairs_list = Element::list(
            FocusId::new("examples-list"),
            &self.pairs,
            &self.list_state,
            theme,
        )
        .on_select(select_handler)
        .on_navigate(list_handler)
        .build();

        // Input panels
        let source_panel = Element::panel(source_input)
            .title("Source Record ID")
            .build();

        let target_panel = Element::panel(target_input)
            .title("Target Record ID")
            .build();

        let label_panel = Element::panel(label_input)
            .title("Label")
            .build();

        let pairs_panel = Element::panel(pairs_list)
            .title("Saved Pairs")
            .build();

        // Buttons (examples are fetched automatically)
        let buttons = button_row![
            ("examples-add", "Add", self.on_add.clone().expect("ExamplesModal requires on_add")),
            ("examples-delete", "Delete", self.on_delete.clone().expect("ExamplesModal requires on_delete")),
            ("examples-close", "Close", self.on_close.clone().expect("ExamplesModal requires on_close")),
        ];

        // Layout with explicit constraints
        let modal_body = col![
            Element::styled_text(
                Line::from(vec![
                    Span::styled("Example Record Pairs", Style::default().fg(theme.accent_tertiary).bold())
                ])
            ).build() => Length(1),
            spacer!() => Length(1),
            source_panel => Length(3),
            spacer!() => Length(1),
            target_panel => Length(3),
            spacer!() => Length(1),
            label_panel => Length(3),
            spacer!() => Length(1),
            pairs_panel => Fill(1),
            spacer!() => Length(1),
            buttons => Length(3),
        ];

        // Wrap in outer panel with title, width, and height
        Element::panel(
            Element::container(modal_body)
                .padding(2)
                .build()
        )
        .width(self.width.unwrap_or(80))
        .height(self.height.unwrap_or(30))
        .build()
    }
}

impl<Msg: Clone> Default for ExamplesModal<Msg> {
    fn default() -> Self {
        Self::new()
    }
}
