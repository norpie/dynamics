//! Import from Excel modals

use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::widgets::ListItem;
use crate::tui::{Element, LayoutConstraint, Theme};

use super::super::state::{Msg, State};

/// Render the import file browser modal
pub fn render_file_browser(state: &State, theme: &Theme) -> Element<Msg> {
    // Current directory display
    let dir_path = state
        .import_file_browser
        .current_path()
        .to_string_lossy()
        .to_string();
    let dir_label = Element::styled_text(Line::from(vec![
        Span::styled("Directory: ", Style::default().fg(theme.text_secondary)),
        Span::styled(
            truncate_str(&dir_path, 60),
            Style::default().fg(theme.text_primary),
        ),
    ]))
    .build();

    // File browser list
    let entries = state.import_file_browser.entries();
    let browser_items: Vec<FileBrowserListItem> = entries
        .iter()
        .map(|entry| FileBrowserListItem {
            name: entry.name.clone(),
            is_dir: entry.is_dir,
            theme: theme.clone(),
        })
        .collect();

    let file_browser = Element::list(
        FocusId::new("import-file-browser"),
        &browser_items,
        state.import_file_browser.list_state(),
        theme,
    )
    .on_navigate(|key| Msg::ImportFileNavigate(key))
    .on_activate(|_| Msg::ImportFileNavigate(KeyCode::Enter))
    .on_render(Msg::ImportSetViewportHeight)
    .build();

    let browser_panel = Element::panel(file_browser)
        .title("Select Excel File (.xlsx)")
        .build();

    // Instructions
    let instructions = Element::styled_text(Line::from(vec![
        Span::styled("Enter", Style::default().fg(theme.accent_primary)),
        Span::styled(
            " to select file or enter directory, ",
            Style::default().fg(theme.text_secondary),
        ),
        Span::styled("Backspace", Style::default().fg(theme.accent_primary)),
        Span::styled(" to go up", Style::default().fg(theme.text_secondary)),
    ]))
    .build();

    // Cancel button
    let cancel_btn = Element::button(FocusId::new("import-cancel"), "Cancel")
        .on_press(Msg::CloseModal)
        .build();

    let button_row = RowBuilder::new()
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(cancel_btn, LayoutConstraint::Length(12))
        .build();

    // Layout
    let content = ColumnBuilder::new()
        .add(dir_label, LayoutConstraint::Length(1))
        .add(browser_panel, LayoutConstraint::Fill(1))
        .add(instructions, LayoutConstraint::Length(1))
        .add(button_row, LayoutConstraint::Length(3))
        .build();

    Element::panel(content)
        .title("Import from Excel")
        .width(80)
        .height(30)
        .build()
}

/// Render the import confirmation modal
pub fn render_confirmation(
    state: &State,
    path: &str,
    conflicts: &[String],
    theme: &Theme,
) -> Element<Msg> {
    let pending = state.pending_import.as_ref();
    let edit_count = pending.map(|p| p.edit_count).unwrap_or(0);
    let conflict_count = conflicts.len();

    // Summary
    let summary = Element::styled_text(Line::from(vec![
        Span::styled("File: ", Style::default().fg(theme.text_secondary)),
        Span::styled(
            truncate_str(path, 55),
            Style::default().fg(theme.text_primary),
        ),
    ]))
    .build();

    let edit_info = Element::styled_text(Line::from(vec![
        Span::styled(
            format!("{} ", edit_count),
            Style::default().fg(theme.accent_primary),
        ),
        Span::styled(
            "records will be modified",
            Style::default().fg(theme.text_primary),
        ),
    ]))
    .build();

    // Conflict warning (if any)
    let conflict_section = if conflict_count > 0 {
        let warning = Element::styled_text(Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(theme.accent_warning)),
            Span::styled(
                format!("{} conflicts detected!", conflict_count),
                Style::default().fg(theme.accent_warning),
            ),
        ]))
        .build();

        let explanation = Element::styled_text(Line::from(vec![
            Span::styled(
                "These records were edited locally and will be ",
                Style::default().fg(theme.text_secondary),
            ),
            Span::styled("overwritten", Style::default().fg(theme.accent_error)),
            Span::styled(":", Style::default().fg(theme.text_secondary)),
        ]))
        .build();

        // List conflicting IDs (show first 5)
        let conflict_list: Vec<Element<Msg>> = conflicts
            .iter()
            .take(5)
            .map(|id| {
                Element::styled_text(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(
                        truncate_str(id, 40),
                        Style::default().fg(theme.text_secondary),
                    ),
                ]))
                .build()
            })
            .collect();

        let mut col = ColumnBuilder::new()
            .add(warning, LayoutConstraint::Length(1))
            .add(Element::text(""), LayoutConstraint::Length(1))
            .add(explanation, LayoutConstraint::Length(1));

        for item in conflict_list {
            col = col.add(item, LayoutConstraint::Length(1));
        }

        if conflict_count > 5 {
            col = col.add(
                Element::styled_text(Line::from(vec![Span::styled(
                    format!("  ... and {} more", conflict_count - 5),
                    Style::default().fg(theme.text_tertiary),
                )]))
                .build(),
                LayoutConstraint::Length(1),
            );
        }

        col.build()
    } else {
        Element::styled_text(Line::from(vec![
            Span::styled("✓ ", Style::default().fg(theme.accent_success)),
            Span::styled(
                "No conflicts with local edits",
                Style::default().fg(theme.accent_success),
            ),
        ]))
        .build()
    };

    // Buttons
    let import_btn = Element::button(FocusId::new("import-confirm"), "Import")
        .on_press(Msg::ConfirmImport)
        .build();
    let cancel_btn = Element::button(FocusId::new("import-cancel"), "Cancel")
        .on_press(Msg::CancelImport)
        .build();

    let button_row = RowBuilder::new()
        .add(import_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(cancel_btn, LayoutConstraint::Length(12))
        .build();

    // Layout
    let content = ColumnBuilder::new()
        .add(summary, LayoutConstraint::Length(1))
        .add(edit_info, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(conflict_section, LayoutConstraint::Fill(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(button_row, LayoutConstraint::Length(3))
        .build();

    Element::panel(content)
        .title("Confirm Import")
        .width(70)
        .height(20)
        .build()
}

/// List item for file browser entries
struct FileBrowserListItem {
    name: String,
    is_dir: bool,
    theme: Theme,
}

impl ListItem for FileBrowserListItem {
    type Msg = Msg;

    fn to_element(
        &self,
        is_selected: bool,
        _is_multi_selected: bool,
        _is_hovered: bool,
    ) -> Element<Self::Msg> {
        let (icon, color) = if self.is_dir {
            ("📁 ", self.theme.accent_secondary)
        } else {
            ("📄 ", self.theme.text_primary)
        };

        let style = if is_selected {
            Style::default().fg(color).bg(self.theme.bg_surface)
        } else {
            Style::default().fg(color)
        };

        let display = format!("{}{}", icon, self.name);

        Element::styled_text(Line::from(Span::styled(display, style))).build()
    }
}

/// Truncate a string to max length with ellipsis (UTF-8 safe)
fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}
