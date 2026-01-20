use crate::tui::element::{ColumnBuilder, LayoutConstraint, RowBuilder};
use crate::tui::{Element, FocusId, Theme};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Builder for error modals with auto-hotkey generation
///
/// # Example
/// ```rust
/// let modal = ErrorModal::new("Failed to load data")
///     .details(error_string)
///     .on_close(Msg::DismissError)
///     .build(theme);
/// ```
pub struct ErrorModal<Msg> {
    title: String,
    details: Option<String>,
    on_close: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> ErrorModal<Msg> {
    /// Create a new error modal with a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            details: None,
            on_close: None,
            width: None,
            height: None,
        }
    }

    /// Set error details/description
    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Set the message sent when closed (Escape)
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
        // Error icon + title
        let theme = &crate::global_runtime_config().theme;
        let title_element = Element::styled_text(Line::from(vec![
            Span::styled("✗ ", Style::default().fg(theme.accent_error).bold()),
            Span::styled(self.title, Style::default().fg(theme.accent_error).bold()),
        ]))
        .build();

        // Details element (if present)
        let details_elements: Vec<(LayoutConstraint, Element<Msg>)> =
            if let Some(details) = self.details {
                vec![
                    (LayoutConstraint::Length(1), Element::text("")),
                    (LayoutConstraint::Min(3), Element::text(details)),
                ]
            } else {
                vec![]
            };

        // Extract message to ensure proper typing
        let close_msg = self
            .on_close
            .clone()
            .expect("ErrorModal requires on_close callback");

        // Close button with hotkey indicator
        let close_button =
            Element::button(FocusId::new("error-close"), "[ Close (Esc) ]".to_string())
                .on_press(close_msg)
                .build();

        // Button row - explicitly set Fill constraints for width distribution
        let button_row = RowBuilder::new()
            .add(Element::text(""), LayoutConstraint::Fill(1))
            .add(close_button, LayoutConstraint::Fill(1))
            .add(Element::text(""), LayoutConstraint::Fill(1))
            .build();

        // Build the modal content
        let mut content = ColumnBuilder::new();
        content = content.add(title_element, LayoutConstraint::Length(1));

        for (constraint, element) in details_elements {
            content = content.add(element, constraint);
        }

        content = content.add(Element::text(""), LayoutConstraint::Length(1));
        content = content.add(button_row, LayoutConstraint::Length(3));

        let content = content.build();

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
