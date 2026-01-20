//! UI building functions for the queue app

use super::app::{ImportModalState, ImportSettings, Msg, State};
use super::models::OperationStatus;
use crate::tui::element::{Element, FocusId, LayoutConstraint};
use crate::tui::widgets::{FileBrowserState, ScrollableState};
use crate::{col, row, use_constraints};
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::{Line as RataLine, Span};

pub fn build_details_panel(state: &State, scroll_state: &ScrollableState) -> Element<Msg> {
    let start = std::time::Instant::now();
    let theme = &crate::global_runtime_config().theme;

    // Check if selected ID is a child node (format: "parent_id_index")
    let (selected_item, child_index) = if let Some(selected_id) = &state.selected_item_id {
        // Try to parse as child ID
        if let Some(last_underscore_pos) = selected_id.rfind('_') {
            let potential_parent_id = &selected_id[..last_underscore_pos];
            let potential_index = &selected_id[last_underscore_pos + 1..];

            // Check if the part after underscore is a number and parent exists
            if let Ok(index) = potential_index.parse::<usize>() {
                if let Some(item) = state
                    .queue_items
                    .iter()
                    .find(|item| item.id == potential_parent_id)
                {
                    (Some(item.clone()), Some(index))
                } else {
                    // Not a valid child, try as parent
                    (
                        state
                            .queue_items
                            .iter()
                            .find(|item| &item.id == selected_id)
                            .cloned(),
                        None,
                    )
                }
            } else {
                // Not a number, must be a parent ID
                (
                    state
                        .queue_items
                        .iter()
                        .find(|item| &item.id == selected_id)
                        .cloned(),
                    None,
                )
            }
        } else {
            // No underscore, must be a parent ID
            (
                state
                    .queue_items
                    .iter()
                    .find(|item| &item.id == selected_id)
                    .cloned(),
                None,
            )
        }
    } else {
        (None, None)
    };

    let content = if let Some(item) = selected_item {
        // If viewing a child node, show details about that specific operation
        if let Some(child_idx) = child_index {
            build_operation_details(&item, child_idx, theme)
        } else {
            build_batch_overview(&item, theme)
        }
    } else {
        // No selection
        Element::column(vec![
            Element::styled_text(RataLine::from(vec![Span::styled(
                "No item selected",
                Style::default().fg(theme.border_primary).italic(),
            )]))
            .build(),
        ])
        .spacing(0)
        .build()
    };

    // Wrap content in scrollable
    let scrollable_content =
        Element::scrollable(FocusId::new("details-scroll"), content, scroll_state)
            .on_navigate(Msg::DetailsScroll)
            .on_render(Msg::DetailsSetDimensions)
            .build();

    let elapsed = start.elapsed();
    if elapsed.as_micros() > 500 {
        log::warn!("PERF build_details_panel: {}us", elapsed.as_micros());
    }

    Element::panel(scrollable_content).title("Details").build()
}

fn build_operation_details(
    item: &super::models::QueueItem,
    child_idx: usize,
    theme: &crate::tui::state::theme::Theme,
) -> Element<Msg> {
    // Get the specific operation (child_idx is 1-based from tree_nodes.rs, but we skip(1) in the tree)
    // So child_idx=1 means index 1 in the operations array (second operation)
    let operations = item.operations.operations();
    if child_idx >= operations.len() {
        return Element::column(vec![
            Element::styled_text(RataLine::from(vec![Span::styled(
                "Invalid operation index",
                Style::default().fg(theme.accent_error),
            )]))
            .build(),
        ])
        .spacing(0)
        .build();
    }

    let operation = &operations[child_idx];

    let mut lines = vec![
        // Header
        Element::styled_text(RataLine::from(vec![Span::styled(
            format!("Operation {} of {}", child_idx + 1, operations.len()),
            Style::default().fg(theme.text_primary).bold(),
        )]))
        .build(),
        Element::text(""),
        // Parent batch info
        Element::styled_text(RataLine::from(vec![
            Span::styled("Batch: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                item.metadata.description.clone(),
                Style::default().fg(theme.text_primary),
            ),
        ]))
        .build(),
        Element::text(""),
        // Operation type
        Element::styled_text(RataLine::from(vec![
            Span::styled("Type: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                operation.operation_type().to_string(),
                Style::default().fg(theme.accent_secondary),
            ),
        ]))
        .build(),
        // Entity
        Element::styled_text(RataLine::from(vec![
            Span::styled("Entity: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                operation.entity().to_string(),
                Style::default().fg(theme.text_primary),
            ),
        ]))
        .build(),
    ];

    // Construct endpoint
    use crate::api::operations::Operation;
    let endpoint = match operation {
        Operation::Create { entity, .. } | Operation::CreateWithRefs { entity, .. } => {
            format!("POST /{}", entity)
        }
        Operation::Update { entity, id, .. } => {
            format!("PATCH /{}({})", entity, id)
        }
        Operation::Delete { entity, id, .. } => {
            format!("DELETE /{}({})", entity, id)
        }
        Operation::Upsert {
            entity,
            key_field,
            key_value,
            ..
        } => {
            format!("PATCH /{}({}='{}')", entity, key_field, key_value)
        }
        Operation::AssociateRef {
            entity,
            entity_ref,
            navigation_property,
            ..
        } => {
            format!(
                "POST /{}({})/{}/$ref",
                entity, entity_ref, navigation_property
            )
        }
        Operation::DisassociateRef {
            entity,
            entity_ref,
            navigation_property,
            target_id,
        } => {
            format!(
                "DELETE /{}({})/{}({})/$ref",
                entity, entity_ref, navigation_property, target_id
            )
        }
        // Schema operations
        Operation::CreateAttribute { entity, .. } => {
            format!("POST /EntityDefinitions('{}')/Attributes", entity)
        }
        Operation::UpdateAttribute {
            entity, attribute, ..
        } => {
            format!(
                "PUT /EntityDefinitions('{}')/Attributes('{}')",
                entity, attribute
            )
        }
        Operation::DeleteAttribute { entity, attribute } => {
            format!(
                "DELETE /EntityDefinitions('{}')/Attributes('{}')",
                entity, attribute
            )
        }
        Operation::PublishAllXml => "POST /PublishAllXml".to_string(),
    };

    lines.push(
        Element::styled_text(RataLine::from(vec![
            Span::styled("Endpoint: ", Style::default().fg(theme.border_primary)),
            Span::styled(endpoint, Style::default().fg(theme.accent_secondary)),
        ]))
        .build(),
    );

    // Show data based on operation type
    match operation {
        Operation::Create { data, .. }
        | Operation::CreateWithRefs { data, .. }
        | Operation::Update { data, .. }
        | Operation::Upsert { data, .. } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Data:",
                    Style::default().fg(theme.accent_muted).bold(),
                )]))
                .build(),
            );

            // Pretty print JSON data
            if let Ok(json_str) = serde_json::to_string_pretty(data) {
                for line in json_str.lines() {
                    lines.push(
                        Element::styled_text(RataLine::from(vec![Span::styled(
                            format!("  {}", line),
                            Style::default().fg(theme.text_primary),
                        )]))
                        .build(),
                    );
                }
            }
        }
        Operation::Delete { id, .. } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Record ID: ", Style::default().fg(theme.border_primary)),
                    Span::styled(id.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
            );
        }
        Operation::AssociateRef {
            entity_ref,
            navigation_property,
            target_ref,
            ..
        } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Entity Ref: ", Style::default().fg(theme.border_primary)),
                    Span::styled(entity_ref.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
            );
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Navigation: ", Style::default().fg(theme.border_primary)),
                    Span::styled(
                        navigation_property.clone(),
                        Style::default().fg(theme.text_primary),
                    ),
                ]))
                .build(),
            );
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Target: ", Style::default().fg(theme.border_primary)),
                    Span::styled(target_ref.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
            );
        }
        Operation::DisassociateRef {
            entity_ref,
            navigation_property,
            target_id,
            ..
        } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Entity Ref: ", Style::default().fg(theme.border_primary)),
                    Span::styled(entity_ref.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
            );
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Navigation: ", Style::default().fg(theme.border_primary)),
                    Span::styled(
                        navigation_property.clone(),
                        Style::default().fg(theme.text_primary),
                    ),
                ]))
                .build(),
            );
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Target ID: ", Style::default().fg(theme.border_primary)),
                    Span::styled(target_id.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
            );
        }
        // Schema operations
        Operation::CreateAttribute {
            attribute_data,
            solution_name,
            ..
        } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Attribute Data:",
                    Style::default().fg(theme.accent_muted).bold(),
                )]))
                .build(),
            );

            if let Ok(json_str) = serde_json::to_string_pretty(attribute_data) {
                for line in json_str.lines() {
                    lines.push(
                        Element::styled_text(RataLine::from(vec![Span::styled(
                            format!("  {}", line),
                            Style::default().fg(theme.text_primary),
                        )]))
                        .build(),
                    );
                }
            }

            // Show solution name if present
            if let Some(solution) = solution_name {
                lines.push(
                    Element::styled_text(RataLine::from(vec![
                        Span::styled("Solution: ", Style::default().fg(theme.border_primary)),
                        Span::styled(solution.clone(), Style::default().fg(theme.text_primary)),
                    ]))
                    .build(),
                );
            }
        }
        Operation::UpdateAttribute { attribute_data, .. } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Attribute Data:",
                    Style::default().fg(theme.accent_muted).bold(),
                )]))
                .build(),
            );

            if let Ok(json_str) = serde_json::to_string_pretty(attribute_data) {
                for line in json_str.lines() {
                    lines.push(
                        Element::styled_text(RataLine::from(vec![Span::styled(
                            format!("  {}", line),
                            Style::default().fg(theme.text_primary),
                        )]))
                        .build(),
                    );
                }
            }
        }
        Operation::DeleteAttribute { attribute, .. } => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("Attribute: ", Style::default().fg(theme.border_primary)),
                    Span::styled(attribute.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
            );
        }
        Operation::PublishAllXml => {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Publishes all pending customizations",
                    Style::default().fg(theme.border_primary).italic(),
                )]))
                .build(),
            );
        }
    }

    // Show result if operation completed
    if let Some(result) = &item.result {
        if child_idx < result.operation_results.len() {
            let op_result = &result.operation_results[child_idx];

            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Result:",
                    Style::default().fg(theme.accent_muted).bold(),
                )]))
                .build(),
            );

            let status_color = if op_result.success {
                theme.accent_success
            } else {
                theme.accent_error
            };
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled("  Status: ", Style::default().fg(theme.border_primary)),
                    Span::styled(
                        if op_result.success {
                            "Success"
                        } else {
                            "Failed"
                        },
                        Style::default().fg(status_color),
                    ),
                ]))
                .build(),
            );

            if let Some(status_code) = op_result.status_code {
                lines.push(
                    Element::styled_text(RataLine::from(vec![
                        Span::styled("  Status Code: ", Style::default().fg(theme.border_primary)),
                        Span::styled(
                            status_code.to_string(),
                            Style::default().fg(theme.text_primary),
                        ),
                    ]))
                    .build(),
                );
            }

            if let Some(error) = &op_result.error {
                lines.push(Element::text(""));
                lines.push(
                    Element::styled_text(RataLine::from(vec![Span::styled(
                        "  Error:",
                        Style::default().fg(theme.accent_error).bold(),
                    )]))
                    .build(),
                );

                for error_line in error.lines() {
                    lines.push(
                        Element::styled_text(RataLine::from(vec![Span::styled(
                            format!("    {}", error_line),
                            Style::default().fg(theme.accent_error),
                        )]))
                        .build(),
                    );
                }
            }
        }
    }

    Element::column(lines).spacing(0).build()
}

fn build_batch_overview(
    item: &super::models::QueueItem,
    theme: &crate::tui::state::theme::Theme,
) -> Element<Msg> {
    let mut lines = vec![
        // Header with status
        Element::styled_text(RataLine::from(vec![
            Span::styled(
                format!("{} ", item.status.symbol()),
                Style::default().fg(match item.status {
                    OperationStatus::Pending => theme.accent_warning,
                    OperationStatus::Running => theme.accent_secondary,
                    OperationStatus::Paused => theme.border_primary,
                    OperationStatus::Done => theme.accent_success,
                    OperationStatus::Failed => theme.accent_error,
                    OperationStatus::PartiallyFailed => theme.accent_warning,
                }),
            ),
            Span::styled(
                item.metadata.description.clone(),
                Style::default().fg(theme.text_primary).bold(),
            ),
        ]))
        .build(),
        Element::text(""),
        // Priority
        Element::styled_text(RataLine::from(vec![
            Span::styled("Priority: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                item.priority.to_string(),
                Style::default().fg(theme.accent_tertiary),
            ),
        ]))
        .build(),
        // Source
        Element::styled_text(RataLine::from(vec![
            Span::styled("Source: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                item.metadata.source.clone(),
                Style::default().fg(theme.text_primary),
            ),
        ]))
        .build(),
        // Entity type
        Element::styled_text(RataLine::from(vec![
            Span::styled("Entity: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                item.metadata.entity_type.clone(),
                Style::default().fg(theme.text_primary),
            ),
        ]))
        .build(),
        // Environment
        Element::styled_text(RataLine::from(vec![
            Span::styled("Environment: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                item.metadata.environment_name.clone(),
                Style::default().fg(theme.text_primary),
            ),
        ]))
        .build(),
    ];

    // Row number if applicable
    if let Some(row) = item.metadata.row_number {
        lines.push(
            Element::styled_text(RataLine::from(vec![
                Span::styled("Row: ", Style::default().fg(theme.border_primary)),
                Span::styled(row.to_string(), Style::default().fg(theme.text_primary)),
            ]))
            .build(),
        );
    }

    // Warning section if interrupted
    if item.was_interrupted {
        lines.push(Element::text(""));
        lines.push(
            Element::styled_text(RataLine::from(vec![
                Span::styled(
                    "⚠ WARNING: ",
                    Style::default().fg(theme.accent_error).bold(),
                ),
                Span::styled(
                    "Operation was interrupted and may have partially executed.",
                    Style::default().fg(theme.accent_warning),
                ),
            ]))
            .build(),
        );

        if let Some(interrupted_at) = item.interrupted_at {
            lines.push(
                Element::styled_text(RataLine::from(vec![
                    Span::styled(
                        "  Interrupted at: ",
                        Style::default().fg(theme.border_primary),
                    ),
                    Span::styled(
                        interrupted_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                        Style::default().fg(theme.text_primary),
                    ),
                ]))
                .build(),
            );
        }

        lines.push(
            Element::styled_text(RataLine::from(vec![Span::styled(
                "  → Verify completion in Dynamics before retrying or deleting",
                Style::default().fg(theme.accent_warning).italic(),
            )]))
            .build(),
        );

        // Add clear warning button
        let clear_warning_btn = Element::button(
            FocusId::new("clear-warning"),
            "[c] Mark as Verified".to_string(),
        )
        .on_press(Msg::ClearInterruptionFlag(item.id.clone()))
        .build();

        lines.push(Element::text(""));
        lines.push(clear_warning_btn);
    }

    lines.push(Element::text(""));

    // Operations list
    lines.push(
        Element::styled_text(RataLine::from(vec![Span::styled(
            format!("Operations ({}):", item.operations.len()),
            Style::default().fg(theme.accent_muted).bold(),
        )]))
        .build(),
    );

    for (idx, op) in item.operations.operations().iter().enumerate() {
        let op_type = op.operation_type().to_string();
        let entity = op.entity().to_string();

        lines.push(
            Element::styled_text(RataLine::from(vec![
                Span::styled(
                    format!("  {}. ", idx + 1),
                    Style::default().fg(theme.border_primary),
                ),
                Span::styled(op_type, Style::default().fg(theme.accent_secondary)),
                Span::raw(" "),
                Span::styled(entity, Style::default().fg(theme.text_primary)),
            ]))
            .build(),
        );
    }

    // Show results if completed or failed
    if let Some(result) = &item.result {
        lines.push(Element::text(""));
        lines.push(
            Element::styled_text(RataLine::from(vec![Span::styled(
                "Result:",
                Style::default().fg(theme.accent_muted).bold(),
            )]))
            .build(),
        );

        lines.push(
            Element::styled_text(RataLine::from(vec![
                Span::styled("  Status: ", Style::default().fg(theme.border_primary)),
                Span::styled(
                    if result.success { "Success" } else { "Failed" },
                    Style::default().fg(if result.success {
                        theme.accent_success
                    } else {
                        theme.accent_error
                    }),
                ),
            ]))
            .build(),
        );

        lines.push(
            Element::styled_text(RataLine::from(vec![
                Span::styled("  Duration: ", Style::default().fg(theme.border_primary)),
                Span::styled(
                    format!("{}ms", result.duration_ms),
                    Style::default().fg(theme.text_primary),
                ),
            ]))
            .build(),
        );

        // Show error if any
        if let Some(error) = &result.error {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Error:",
                    Style::default().fg(theme.accent_error).bold(),
                )]))
                .build(),
            );

            // Split error message into lines if too long
            let max_width = 40;
            let error_lines: Vec<&str> = error
                .as_str()
                .split('\n')
                .flat_map(|line| {
                    if line.len() <= max_width {
                        vec![line]
                    } else {
                        line.as_bytes()
                            .chunks(max_width)
                            .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
                            .collect()
                    }
                })
                .collect();

            for error_line in error_lines {
                lines.push(
                    Element::styled_text(RataLine::from(vec![Span::styled(
                        format!("  {}", error_line),
                        Style::default().fg(theme.accent_error),
                    )]))
                    .build(),
                );
            }
        }

        // Show individual operation results
        if !result.operation_results.is_empty() {
            lines.push(Element::text(""));
            lines.push(
                Element::styled_text(RataLine::from(vec![Span::styled(
                    "Operation Results:",
                    Style::default().fg(theme.accent_muted).bold(),
                )]))
                .build(),
            );

            for (idx, op_result) in result.operation_results.iter().enumerate() {
                let status_symbol = if op_result.success { "✓" } else { "✗" };
                let status_color = if op_result.success {
                    theme.accent_success
                } else {
                    theme.accent_error
                };

                let msg = if let Some(err) = &op_result.error {
                    err.clone()
                } else {
                    "OK".to_string()
                };

                lines.push(
                    Element::styled_text(RataLine::from(vec![
                        Span::styled(
                            format!("  {}. ", idx + 1),
                            Style::default().fg(theme.border_primary),
                        ),
                        Span::styled(status_symbol, Style::default().fg(status_color)),
                        Span::raw(" "),
                        Span::styled(msg, Style::default().fg(theme.text_primary)),
                    ]))
                    .build(),
                );
            }
        }
    }

    Element::column(lines).spacing(0).build()
}

pub fn build_clear_confirm_modal() -> Element<Msg> {
    use crate::tui::modals::ConfirmationModal;

    ConfirmationModal::new("Clear Queue")
        .message("Are you sure you want to clear all queue items?\nThis action cannot be undone.")
        .confirm_text("Yes")
        .cancel_text("No")
        .on_confirm(Msg::ConfirmClearQueue)
        .on_cancel(Msg::CancelModal)
        .width(60)
        .build()
}

pub fn build_delete_confirm_modal() -> Element<Msg> {
    use crate::tui::modals::ConfirmationModal;

    ConfirmationModal::new("Delete Item")
        .message("Are you sure you want to delete this queue item?\nThis action cannot be undone.")
        .confirm_text("Yes")
        .cancel_text("No")
        .on_confirm(Msg::ConfirmDeleteSelected)
        .on_cancel(Msg::CancelModal)
        .width(60)
        .build()
}

pub fn build_interruption_warning_modal(state: &State) -> Element<Msg> {
    use crate::tui::modals::WarningModal;

    let interrupted_items = if let Some(items) = state.interruption_warning_modal.data() {
        items
    } else {
        return Element::None; // Should not happen
    };

    let count = interrupted_items.len();
    let message = format!(
        "The application was closed while {} operation(s) were executing.\n\
        These may have partially completed in Dynamics 365.\n\
        \n\
        Before resuming:\n\
        • Verify in Dynamics whether operations succeeded\n\
        • Delete items that already completed (press 'd')\n\
        • Keep items that need retry\n\
        \n\
        Items are marked with ⚠ in the queue.\n\
        Press 'c' on an item to clear its warning.",
        count
    );

    let mut modal = WarningModal::new("Interrupted Operations Detected")
        .message(message)
        .on_close(Msg::DismissInterruptionWarning)
        .width(80);

    // Add first few items as examples (limit to 5)
    for item in interrupted_items.iter().take(5) {
        let item_desc = format!(
            "{} ({})",
            item.metadata.description, item.metadata.environment_name
        );
        modal = modal.add_item(item_desc);
    }

    if interrupted_items.len() > 5 {
        modal = modal.add_item(format!("... and {} more", interrupted_items.len() - 5));
    }

    modal.build()
}

/// Build import file browser modal
pub fn build_import_file_browser(browser: &FileBrowserState) -> Element<Msg> {
    use_constraints!();
    let theme = &crate::global_runtime_config().theme;

    // Header with current path
    let path_label = Element::styled_text(RataLine::from(vec![Span::styled(
        "Path:",
        Style::default().fg(theme.accent_muted).bold(),
    )]))
    .build();

    let path_value = Element::styled_text(RataLine::from(vec![Span::styled(
        browser.current_path().display().to_string(),
        Style::default().fg(theme.text_primary),
    )]))
    .build();

    // File browser widget wrapped in panel
    let file_list = Element::file_browser("import-browser", browser, theme)
        .on_navigate(Msg::ImportFileNavigate)
        .build();

    let file_panel = Element::panel(file_list).title("Files").build();

    // Instructions
    let instructions = Element::styled_text(RataLine::from(vec![
        Span::styled("↑↓ ", Style::default().fg(theme.accent_tertiary)),
        Span::styled("Navigate  ", Style::default().fg(theme.border_primary)),
        Span::styled("Enter ", Style::default().fg(theme.accent_tertiary)),
        Span::styled("Select/Open  ", Style::default().fg(theme.border_primary)),
        Span::styled("Backspace ", Style::default().fg(theme.accent_tertiary)),
        Span::styled("Go up  ", Style::default().fg(theme.border_primary)),
        Span::styled("Esc ", Style::default().fg(theme.accent_tertiary)),
        Span::styled("Cancel", Style::default().fg(theme.border_primary)),
    ]))
    .build();

    // Cancel button
    let cancel_btn = Element::button("import-cancel", " Cancel ")
        .on_press(Msg::ImportCancel)
        .build();

    let footer = row![
        instructions => Fill(1),
        cancel_btn => Length(12),
    ];

    let content = col![
        path_label => Length(1),
        path_value => Length(1),
        Element::None => Length(1),
        file_panel => Fill(1),
        Element::None => Length(1),
        footer => Length(3),
    ];

    Element::panel(content)
        .title("Select Excel File (.xlsx)")
        .build()
}

/// Build import settings modal
pub fn build_import_settings(settings: &ImportSettings) -> Element<Msg> {
    use_constraints!();
    let theme = &crate::global_runtime_config().theme;

    // File info
    let file_info = Element::styled_text(RataLine::from(vec![
        Span::styled("File: ", Style::default().fg(theme.border_primary)),
        Span::styled(
            settings
                .file_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
            Style::default().fg(theme.text_primary),
        ),
    ]))
    .build();

    // Operation summary
    let mut summary_lines = vec![
        Element::styled_text(RataLine::from(vec![Span::styled(
            "Operations:",
            Style::default().fg(theme.accent_muted).bold(),
        )]))
        .build(),
    ];

    for sheet in &settings.parsed.sheets {
        summary_lines.push(
            Element::styled_text(RataLine::from(vec![
                Span::styled("  • ", Style::default().fg(theme.border_primary)),
                Span::styled(
                    format!(
                        "{} {}: {} operations",
                        sheet.operation_type,
                        sheet.entity,
                        sheet.operations.len()
                    ),
                    Style::default().fg(theme.text_primary),
                ),
            ]))
            .build(),
        );
    }

    summary_lines.push(
        Element::styled_text(RataLine::from(vec![
            Span::styled("  Total: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                format!("{} operations", settings.parsed.total_count),
                Style::default().fg(theme.accent_success),
            ),
        ]))
        .build(),
    );

    // Calculate batches
    let total_batches: usize = settings
        .parsed
        .sheets
        .iter()
        .map(|s| (s.operations.len() + settings.batch_size - 1) / settings.batch_size)
        .sum();

    summary_lines.push(
        Element::styled_text(RataLine::from(vec![
            Span::styled("  Batches: ", Style::default().fg(theme.border_primary)),
            Span::styled(
                format!("{} queue items", total_batches),
                Style::default().fg(theme.text_primary),
            ),
        ]))
        .build(),
    );

    let summary = Element::column(summary_lines).spacing(0).build();

    // Batch size control - wrap in panel for proper sizing
    let batch_dec = Element::button("batch-dec", " - ")
        .on_press(Msg::ImportSetBatchSize(
            settings.batch_size.saturating_sub(10).max(1),
        ))
        .build();
    let batch_inc = Element::button("batch-inc", " + ")
        .on_press(Msg::ImportSetBatchSize(
            (settings.batch_size + 10).min(1000),
        ))
        .build();
    let batch_value = Element::styled_text(RataLine::from(vec![Span::styled(
        format!("{}", settings.batch_size),
        Style::default().fg(theme.text_primary),
    )]))
    .build();

    let batch_controls = row![
        batch_dec => Length(7),
        Element::None => Length(1),
        batch_value => Fill(1),
        Element::None => Length(1),
        batch_inc => Length(7),
    ];
    let batch_panel = Element::panel(batch_controls).title("Batch Size").build();

    // Priority control - wrap in panel
    let priority_dec = Element::button("priority-dec", " - ")
        .on_press(Msg::ImportSetPriority(settings.priority.saturating_sub(1)))
        .build();
    let priority_inc = Element::button("priority-inc", " + ")
        .on_press(Msg::ImportSetPriority(settings.priority.saturating_add(1)))
        .build();
    let priority_value = Element::styled_text(RataLine::from(vec![Span::styled(
        format!("{}", settings.priority),
        Style::default().fg(theme.text_primary),
    )]))
    .build();

    let priority_controls = row![
        priority_dec => Length(7),
        Element::None => Length(1),
        priority_value => Fill(1),
        Element::None => Length(1),
        priority_inc => Length(7),
    ];
    let priority_panel = Element::panel(priority_controls).title("Priority").build();

    // Environment selector - wrap in panel
    let current_idx = settings
        .available_environments
        .iter()
        .position(|e| e == &settings.environment)
        .unwrap_or(0);
    let prev_idx = if current_idx == 0 {
        settings.available_environments.len().saturating_sub(1)
    } else {
        current_idx - 1
    };
    let next_idx = (current_idx + 1) % settings.available_environments.len().max(1);

    let prev_env = settings
        .available_environments
        .get(prev_idx)
        .cloned()
        .unwrap_or_else(|| settings.environment.clone());
    let next_env = settings
        .available_environments
        .get(next_idx)
        .cloned()
        .unwrap_or_else(|| settings.environment.clone());

    let env_prev = Element::button("env-prev", " < ")
        .on_press(Msg::ImportSetEnvironment(prev_env))
        .build();
    let env_next = Element::button("env-next", " > ")
        .on_press(Msg::ImportSetEnvironment(next_env))
        .build();
    let env_value = Element::styled_text(RataLine::from(vec![Span::styled(
        settings.environment.clone(),
        Style::default().fg(theme.accent_secondary),
    )]))
    .build();

    let env_controls = row![
        env_prev => Length(7),
        Element::None => Length(1),
        env_value => Fill(1),
        Element::None => Length(1),
        env_next => Length(7),
    ];
    let env_panel = Element::panel(env_controls).title("Environment").build();

    // Action buttons
    let back_btn = Element::button("import-back", " Cancel ")
        .on_press(Msg::ImportCancel)
        .build();
    let confirm_btn = Element::button("import-confirm", " Import ")
        .on_press(Msg::ImportConfirm)
        .build();

    let buttons = row![
        back_btn => Length(12),
        Element::None => Fill(1),
        confirm_btn => Length(12),
    ];

    // Summary height: 1 (header) + sheets + 2 (total + batches)
    let summary_height = (settings.parsed.sheets.len() + 3) as u16;

    let content = col![
        file_info => Length(1),
        Element::None => Length(1),
        summary => Length(summary_height),
        Element::None => Length(1),
        batch_panel => Length(5),      // panel: 2 border + 3 content
        priority_panel => Length(5),
        env_panel => Length(5),
        Element::None => Length(1),
        buttons => Length(3),          // button: 2 border + 1 content
    ];

    Element::panel(content).title("Import Settings").build()
}

/// Build import confirmation modal (currently unused - settings modal has confirm)
pub fn build_import_confirmation(settings: &ImportSettings) -> Element<Msg> {
    use crate::tui::modals::ConfirmationModal;

    let batches = settings
        .parsed
        .sheets
        .iter()
        .map(|s| (s.operations.len() + settings.batch_size - 1) / settings.batch_size)
        .sum::<usize>();

    let message = format!(
        "Ready to import {} operations in {} batches.\n\
        Entity: {}\n\
        Environment: {}\n\
        Priority: {}",
        settings.parsed.total_count,
        batches,
        settings
            .parsed
            .sheets
            .iter()
            .map(|s| s.entity.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        settings.environment,
        settings.priority
    );

    ConfirmationModal::new("Confirm Import")
        .message(message)
        .confirm_text("Import")
        .cancel_text("Cancel")
        .on_confirm(Msg::ImportConfirm)
        .on_cancel(Msg::ImportCancel)
        .width(60)
        .build()
}
