use crate::tui::element::{ColumnBuilder, LayoutConstraint, RowBuilder};
use crate::tui::{Element, FocusId, Theme};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Builder for warning modals with optional item list
///
/// # Example
/// ```rust
/// let modal = WarningModal::new("Interrupted Operations")
///     .message("The following operations were interrupted...")
///     .items(vec!["Item 1", "Item 2"])
///     .on_close(Msg::DismissWarning)
///     .build(theme);
/// ```
pub struct WarningModal<Msg> {
    title: String,
    message: Option<String>,
    items: Vec<String>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> WarningModal<Msg> {
    /// Create a new warning modal with a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: None,
            items: Vec::new(),
            on_close: None,
            width: None,
            height: None,
        }
    }

    /// Set warning message/description
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set list of items to display
    pub fn items(mut self, items: Vec<String>) -> Self {
        self.items = items;
        self
    }

    /// Add a single item to the list
    pub fn add_item(mut self, item: impl Into<String>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Set the message sent when closed (Escape or button)
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Set modal width (optional, auto-sizes by default)
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set modal height (optional, auto-sizes by default)
    pub fn height(mut self, height: u16) -> Self {
        self.height = Some(height);
        self
    }

    /// Build the modal Element
    pub fn build(self) -> Element<Msg> {
        // Warning icon + title
        let theme = &crate::global_runtime_config().theme;
        let title_element = Element::styled_text(Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(theme.accent_warning).bold()),
            Span::styled(self.title, Style::default().fg(theme.accent_warning).bold()),
        ]))
        .build();

        let mut content_elements: Vec<(LayoutConstraint, Element<Msg>)> =
            vec![(LayoutConstraint::Length(1), title_element)];

        // Message element (if present)
        if let Some(msg) = self.message {
            content_elements.push((LayoutConstraint::Length(1), Element::text("")));

            // Split message by newlines and add each line
            for line in msg.split('\n') {
                content_elements.push((
                    LayoutConstraint::Length(1),
                    Element::styled_text(Line::from(vec![Span::styled(
                        line.to_string(),
                        Style::default().fg(theme.text_primary),
                    )]))
                    .build(),
                ));
            }
        }

        // Items list (if any)
        if !self.items.is_empty() {
            content_elements.push((LayoutConstraint::Length(1), Element::text("")));

            for item in &self.items {
                content_elements.push((
                    LayoutConstraint::Length(1),
                    Element::styled_text(Line::from(vec![
                        Span::styled("  • ", Style::default().fg(theme.accent_warning)),
                        Span::styled(item.clone(), Style::default().fg(theme.text_primary)),
                    ]))
                    .build(),
                ));
            }
        }

        // Extract message to ensure proper typing
        let close_msg = self
            .on_close
            .clone()
            .expect("WarningModal requires on_close callback");

        // Close button with hotkey indicator
        let close_button = Element::button(FocusId::new("warning-close"), "[ OK ]".to_string())
            .on_press(close_msg)
            .build();

        // Button row - centered
        let button_row = RowBuilder::new()
            .add(Element::text(""), LayoutConstraint::Fill(1))
            .add(close_button, LayoutConstraint::Length(8))
            .add(Element::text(""), LayoutConstraint::Fill(1))
            .build();

        content_elements.push((LayoutConstraint::Length(1), Element::text("")));
        content_elements.push((LayoutConstraint::Length(3), button_row));

        // Build the modal content
        let mut content_builder = ColumnBuilder::new();
        for (constraint, element) in content_elements {
            content_builder = content_builder.add(element, constraint);
        }
        let content = content_builder.build();

        // Wrap in panel with optional size
        let mut panel = Element::panel(content);

        if let Some(w) = self.width {
            panel = panel.width(w);
        }
        if let Some(h) = self.height {
            panel = panel.height(h);
        }

        panel.build()
    }
}
