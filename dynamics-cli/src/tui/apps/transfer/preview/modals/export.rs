//! Export to Excel modal

use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::widgets::ListItem;
use crate::tui::{Element, LayoutConstraint, Theme};

use super::super::state::{Msg, State};

/// Render the export modal with file browser and filename input
pub fn render(state: &State, theme: &Theme) -> Element<Msg> {
    // Current directory display
    let dir_path = state
        .export_file_browser
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
    let entries = state.export_file_browser.entries();
    let browser_items: Vec<FileBrowserListItem> = entries
        .iter()
        .map(|entry| FileBrowserListItem {
            name: entry.name.clone(),
            is_dir: entry.is_dir,
            theme: theme.clone(),
        })
        .collect();

    let file_browser = Element::list(
        FocusId::new("export-file-browser"),
        &browser_items,
        state.export_file_browser.list_state(),
        theme,
    )
    .on_navigate(|key| Msg::ExportFileNavigate(key))
    .on_activate(|_| Msg::ExportFileNavigate(KeyCode::Enter))
    .on_render(Msg::ExportSetViewportHeight)
    .build();

    let browser_panel = Element::panel(file_browser)
        .title("Select Directory")
        .build();

    // Filename input
    let filename_input = Element::text_input(
        FocusId::new("export-filename"),
        state.export_filename.value(),
        &state.export_filename.state,
    )
    .on_event(|e| Msg::ExportFilenameChanged(e))
    .placeholder("filename.xlsx")
    .build();

    let filename_panel = Element::panel(filename_input).title("Filename").build();

    // Full path preview
    let full_path = state
        .export_file_browser
        .current_path()
        .join(state.export_filename.value())
        .to_string_lossy()
        .to_string();

    // Buttons
    let export_btn = Element::button(FocusId::new("export-confirm"), "Export")
        .on_press(Msg::ConfirmExport)
        .build();
    let cancel_btn = Element::button(FocusId::new("export-cancel"), "Cancel")
        .on_press(Msg::CloseModal)
        .build();

    let button_row = RowBuilder::new()
        .add(export_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(cancel_btn, LayoutConstraint::Length(12))
        .build();

    // Layout
    let content = ColumnBuilder::new()
        .add(dir_label, LayoutConstraint::Length(1))
        .add(browser_panel, LayoutConstraint::Fill(1))
        .add(filename_panel, LayoutConstraint::Length(3))
        .add(button_row, LayoutConstraint::Length(3))
        .build();

    Element::panel(content)
        .title("Export to Excel")
        .width(80)
        .height(30)
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

        // Clone name to avoid lifetime issues with the returned Element
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
