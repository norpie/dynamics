use crate::tui::element::{ColumnBuilder, LayoutConstraint, RowBuilder};
use crate::tui::{Element, FocusId, Theme};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};

/// Builder for confirmation modals with auto-hotkey generation
///
/// # Example
/// ```rust
/// let modal = ConfirmationModal::new("Delete migration?")
///     .message("This action cannot be undone")
///     .confirm_text("Delete")
///     .cancel_text("Cancel")
///     .on_confirm(Msg::ConfirmDelete)
///     .on_cancel(Msg::CancelDelete)
///     .build(theme);
/// ```
pub struct ConfirmationModal<Msg> {
    title: String,
    message: Option<String>,
    confirm_text: String,
    cancel_text: String,
    on_confirm: Option<Msg>,
    on_cancel: Option<Msg>,
    width: Option<u16>,
    height: Option<u16>,
}

impl<Msg: Clone> ConfirmationModal<Msg> {
    /// Create a new confirmation modal with a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: None,
            confirm_text: "Confirm".to_string(),
            cancel_text: "Cancel".to_string(),
            on_confirm: None,
            on_cancel: None,
            width: None,
            height: None,
        }
    }

    /// Set the modal message/description
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set the confirm button text (default: "Confirm")
    pub fn confirm_text(mut self, text: impl Into<String>) -> Self {
        self.confirm_text = text.into();
        self
    }

    /// Set the cancel button text (default: "Cancel")
    pub fn cancel_text(mut self, text: impl Into<String>) -> Self {
        self.cancel_text = text.into();
        self
    }

    /// Set the message sent when confirmed
    pub fn on_confirm(mut self, msg: Msg) -> Self {
        self.on_confirm = Some(msg);
        self
    }

    /// Set the message sent when canceled (Escape)
    pub fn on_cancel(mut self, msg: Msg) -> Self {
        self.on_cancel = Some(msg);
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

    /// Generate hotkey display from text (e.g., "Yes" -> "(y)es")
    fn generate_hotkey_label(text: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        let mut chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return text.to_string();
        }

        // Make first char lowercase and wrap in parens
        let first = chars[0].to_lowercase().to_string();
        let rest: String = chars[1..].iter().collect();

        format!("({}){}", first, rest)
    }

    /// Build the modal Element
    pub fn build(self) -> Element<Msg> {
        // Title line
        let theme = &crate::global_runtime_config().theme;
        let title_element = Element::styled_text(Line::from(vec![Span::styled(
            self.title,
            Style::default().fg(theme.accent_tertiary).bold(),
        )]))
        .build();

        // Message element (if present)
        let message_elements: Vec<(LayoutConstraint, Element<Msg>)> =
            if let Some(msg) = self.message {
                vec![
                    (LayoutConstraint::Length(1), Element::text("")),
                    (LayoutConstraint::Length(2), Element::text(msg)),
                ]
            } else {
                vec![]
            };

        // Extract messages to ensure proper typing
        let cancel_msg = self
            .on_cancel
            .clone()
            .expect("ConfirmationModal requires on_cancel callback");
        let confirm_msg = self
            .on_confirm
            .clone()
            .expect("ConfirmationModal requires on_confirm callback");

        // Cancel button with auto-generated hotkey
        let cancel_label = format!("[ {} ]", Self::generate_hotkey_label(&self.cancel_text));
        let cancel_button = Element::button(FocusId::new("confirmation-cancel"), cancel_label)
            .on_press(cancel_msg)
            .build();

        // Confirm button with auto-generated hotkey
        let confirm_label = format!("[ {} ]", Self::generate_hotkey_label(&self.confirm_text));
        let confirm_button = Element::button(FocusId::new("confirmation-confirm"), confirm_label)
            .on_press(confirm_msg)
            .style(Style::default().fg(theme.accent_success))
            .build();

        // Button row - explicitly set Fill constraints for width distribution
        let buttons = RowBuilder::new()
            .add(cancel_button, LayoutConstraint::Fill(1))
            .add(confirm_button, LayoutConstraint::Fill(1))
            .spacing(2)
            .build();

        // Build the modal content
        let mut content = ColumnBuilder::new();
        content = content.add(title_element, LayoutConstraint::Length(1));

        for (constraint, element) in message_elements {
            content = content.add(element, constraint);
        }

        content = content.add(Element::text(""), LayoutConstraint::Length(1));
        content = content.add(buttons, LayoutConstraint::Length(3));

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
