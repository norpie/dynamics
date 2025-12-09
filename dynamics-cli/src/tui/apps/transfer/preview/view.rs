//! View rendering for the Transfer Preview app

use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::element::ColumnBuilder;
use crate::tui::resource::Resource;
use crate::tui::{Alignment, Element, LayeredView, LayoutConstraint, Subscription, Theme};

use super::state::{Msg, PreviewModal, State};

/// Render the preview app view
pub fn render(state: &mut State, theme: &Theme) -> LayeredView<Msg> {
    let content = match &state.resolved {
        Resource::NotAsked => {
            Element::text("No data loaded. Press Back to return to editor.")
        }
        Resource::Loading => {
            Element::text("Loading resolved records...")
        }
        Resource::Failure(err) => {
            Element::styled_text(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(theme.accent_error)),
                Span::styled(err.clone(), Style::default().fg(theme.text_primary)),
            ]))
            .build()
        }
        Resource::Success(resolved) => {
            render_preview(state, resolved, theme)
        }
    };

    // Build header with env info
    let header = Element::styled_text(Line::from(vec![
        Span::styled("Source: ", Style::default().fg(theme.text_secondary)),
        Span::styled(state.source_env.clone(), Style::default().fg(theme.accent_primary)),
        Span::raw(" → "),
        Span::styled("Target: ", Style::default().fg(theme.text_secondary)),
        Span::styled(state.target_env.clone(), Style::default().fg(theme.accent_primary)),
    ]))
    .build();

    let main_content = ColumnBuilder::new()
        .add(header, LayoutConstraint::Length(1))
        .add(content, LayoutConstraint::Fill(1))
        .build();

    let main_view = Element::panel(main_content)
        .title(&format!("Preview: {}", state.config_name))
        .build();

    let mut view = LayeredView::new(main_view);

    // Render modals
    if let Some(modal) = &state.active_modal {
        let modal_element = match modal {
            PreviewModal::RecordDetails { record_idx } => {
                render_record_details_placeholder(*record_idx, theme)
            }
            PreviewModal::EditRecord { record_idx } => {
                render_edit_record_placeholder(*record_idx, theme)
            }
            PreviewModal::BulkActions => {
                render_bulk_actions_placeholder(theme)
            }
            PreviewModal::ExportExcel => {
                render_export_placeholder(theme)
            }
            PreviewModal::ImportExcel => {
                render_import_placeholder(theme)
            }
            PreviewModal::ImportConfirm { path, conflicts } => {
                render_import_confirm_placeholder(path, conflicts, theme)
            }
        };
        view = view.with_app_modal(modal_element, Alignment::Center);
    }

    view
}

/// Render the main preview content when data is loaded
fn render_preview(
    state: &State,
    resolved: &crate::transfer::ResolvedTransfer,
    theme: &Theme,
) -> Element<Msg> {
    if resolved.entities.is_empty() {
        return Element::text("No entities to preview. Add entity mappings in the editor.");
    }

    let entity = &resolved.entities[state.current_entity_idx];

    // Summary line - use owned strings
    let entity_name = entity.entity_name.clone();
    let position = format!(" ({} of {}) | ", state.current_entity_idx + 1, resolved.entities.len());
    let total = format!("{} total", entity.records.len());
    let upsert = format!("{} upsert", entity.upsert_count());
    let nochange = format!("{} nochange", entity.nochange_count());
    let skip = format!("{} skip", entity.skip_count());
    let error = format!("{} error", entity.error_count());

    let summary = Element::styled_text(Line::from(vec![
        Span::styled("Entity: ", Style::default().fg(theme.text_secondary)),
        Span::styled(entity_name, Style::default().fg(theme.accent_primary)),
        Span::raw(position),
        Span::styled(total, Style::default().fg(theme.text_primary)),
        Span::raw(" | "),
        Span::styled(upsert, Style::default().fg(theme.accent_success)),
        Span::raw(" | "),
        Span::styled(nochange, Style::default().fg(theme.text_secondary)),
        Span::raw(" | "),
        Span::styled(skip, Style::default().fg(theme.accent_warning)),
        Span::raw(" | "),
        Span::styled(error, Style::default().fg(theme.accent_error)),
    ]))
    .build();

    // Filter info
    let filter_info = Element::styled_text(Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(theme.text_secondary)),
        Span::styled(state.filter.display_name(), Style::default().fg(theme.accent_primary)),
        Span::raw(" | Press [f] to cycle"),
    ]))
    .build();

    // Placeholder for table (will be implemented in Chunk 4)
    let table_placeholder = Element::text(
        "Table view will be implemented in Chunk 4. Records will appear here."
    );

    ColumnBuilder::new()
        .add(summary, LayoutConstraint::Length(1))
        .add(filter_info, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(table_placeholder, LayoutConstraint::Fill(1))
        .build()
}

// Placeholder modal renderers - will be implemented in later chunks

fn render_record_details_placeholder(record_idx: usize, theme: &Theme) -> Element<Msg> {
    let content = Element::text(&format!(
        "Record Details (index: {})\n\nPress Esc to close.",
        record_idx
    ));

    Element::panel(content)
        .title("Record Details")
        .width(60)
        .height(20)
        .build()
}

fn render_edit_record_placeholder(record_idx: usize, theme: &Theme) -> Element<Msg> {
    let content = Element::text(&format!(
        "Edit Record (index: {})\n\nPress Esc to close.",
        record_idx
    ));

    Element::panel(content)
        .title("Edit Record")
        .width(60)
        .height(20)
        .build()
}

fn render_bulk_actions_placeholder(theme: &Theme) -> Element<Msg> {
    let content = Element::text("Bulk Actions\n\nPress Esc to close.");

    Element::panel(content)
        .title("Bulk Actions")
        .width(50)
        .height(15)
        .build()
}

fn render_export_placeholder(theme: &Theme) -> Element<Msg> {
    let content = Element::text("Export to Excel\n\nPress Esc to close.");

    Element::panel(content)
        .title("Export Excel")
        .width(60)
        .height(20)
        .build()
}

fn render_import_placeholder(theme: &Theme) -> Element<Msg> {
    let content = Element::text("Import from Excel\n\nPress Esc to close.");

    Element::panel(content)
        .title("Import Excel")
        .width(60)
        .height(20)
        .build()
}

fn render_import_confirm_placeholder(path: &str, conflicts: &[String], theme: &Theme) -> Element<Msg> {
    let content = Element::text(&format!(
        "Import Confirmation\n\nPath: {}\nConflicts: {}\n\nPress Esc to close.",
        path,
        conflicts.len()
    ));

    Element::panel(content)
        .title("Confirm Import")
        .width(60)
        .height(20)
        .build()
}

/// Build subscriptions for keyboard shortcuts
pub fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = vec![];

    // Modal-specific subscriptions
    if state.active_modal.is_some() {
        subs.push(Subscription::keyboard(KeyCode::Esc, "Close", Msg::CloseModal));
        return subs;
    }

    // Main view subscriptions
    subs.push(Subscription::keyboard(KeyCode::Esc, "Back to editor", Msg::Back));

    // Entity navigation
    subs.push(Subscription::keyboard(KeyCode::Tab, "Next entity", Msg::NextEntity));
    subs.push(Subscription::shift_key(KeyCode::BackTab, "Previous entity", Msg::PrevEntity));

    // Filtering
    subs.push(Subscription::keyboard(KeyCode::Char('f'), "Cycle filter", Msg::CycleFilter));

    // Record actions
    subs.push(Subscription::keyboard(KeyCode::Enter, "View details", Msg::ViewDetails));
    subs.push(Subscription::keyboard(KeyCode::Char('e'), "Edit record", Msg::EditRecord));
    subs.push(Subscription::keyboard(KeyCode::Char('s'), "Toggle skip", Msg::ToggleSkip));

    // Bulk actions
    subs.push(Subscription::keyboard(KeyCode::Char('b'), "Bulk actions", Msg::OpenBulkActions));

    // Refresh
    subs.push(Subscription::keyboard(KeyCode::Char('r'), "Refresh", Msg::Refresh));

    // Excel
    subs.push(Subscription::keyboard(KeyCode::Char('x'), "Export Excel", Msg::ExportExcel));
    subs.push(Subscription::keyboard(KeyCode::Char('i'), "Import Excel", Msg::ImportExcel));

    // Navigation to execute
    subs.push(Subscription::keyboard(KeyCode::Right, "Go to execute", Msg::GoToExecute));

    subs
}
