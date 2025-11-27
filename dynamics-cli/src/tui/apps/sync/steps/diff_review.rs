//! Step 4: Diff Review View
//!
//! Schema diff display, data preview, and lookup warnings.

use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};

use crate::tui::element::Element;
use crate::tui::widgets::ListItem;
use crate::tui::state::theme::Theme;
use crate::{col, row, spacer, use_constraints, button_row};

use super::super::state::{State, DiffTab};
use super::super::types::{
    EntitySyncPlan, FieldDiffEntry, FieldSyncStatus, DependencyCategory,
    SyncPlan,
};
use super::super::msg::Msg;

/// Entity list item for the left panel
#[derive(Clone)]
struct EntityPlanItem {
    logical_name: String,
    display_name: Option<String>,
    category: DependencyCategory,
    has_changes: bool,
}

impl ListItem for EntityPlanItem {
    type Msg = Msg;

    fn to_element(&self, is_focused: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let category_symbol = self.category.symbol();
        let name = self.display_name.as_ref().unwrap_or(&self.logical_name);
        let change_indicator = if self.has_changes { " ●" } else { " ✓" };

        let text = format!("{} {}{}", category_symbol, name, change_indicator);

        let style = if self.has_changes {
            Style::default().fg(theme.accent_warning)
        } else {
            Style::default().fg(theme.text_primary)
        };

        let bg_style = if is_focused {
            Style::default().bg(theme.bg_surface)
        } else {
            Style::default()
        };

        Element::styled_text(Line::from(Span::styled(text, style)))
            .background(bg_style)
            .build()
    }
}

/// Field diff list item
#[derive(Clone)]
struct FieldDiffItem {
    logical_name: String,
    display_name: Option<String>,
    field_type: String,
    status: FieldSyncStatus,
    is_system_field: bool,
}

impl ListItem for FieldDiffItem {
    type Msg = Msg;

    fn to_element(&self, is_focused: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let icon = match &self.status {
            FieldSyncStatus::InBoth => "✓",
            FieldSyncStatus::OriginOnly => "+",
            FieldSyncStatus::TargetOnly => "⚠",
            FieldSyncStatus::TypeMismatch { .. } => "⚡",
        };

        let display_name = self.display_name.as_ref().unwrap_or(&self.logical_name);

        let mismatch_info = if let FieldSyncStatus::TypeMismatch { origin_type, target_type } = &self.status {
            format!(" ({} → {})", origin_type, target_type)
        } else {
            String::new()
        };

        let system_marker = if self.is_system_field { " [system]" } else { "" };

        let text = format!("{} {} : {}{}{}", icon, display_name, self.field_type, mismatch_info, system_marker);

        let style = match &self.status {
            FieldSyncStatus::InBoth => Style::default().fg(theme.accent_success),
            FieldSyncStatus::OriginOnly => Style::default().fg(theme.accent_info),
            FieldSyncStatus::TargetOnly => Style::default().fg(theme.accent_warning),
            FieldSyncStatus::TypeMismatch { .. } => Style::default().fg(theme.accent_error),
        };

        let bg_style = if is_focused {
            Style::default().bg(theme.bg_surface)
        } else {
            Style::default()
        };

        Element::styled_text(Line::from(Span::styled(text, style)))
            .background(bg_style)
            .build()
    }
}

/// Render the diff review step
pub fn render_diff_review(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let header = render_step_header(state, theme);
    let content = render_diff_content(state, theme);
    let footer = render_step_footer(state, theme);

    col![
        header => Length(3),
        content => Fill(1),
        footer => Length(5),
    ]
}

/// Render step header with tabs
fn render_step_header(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    // Tab bar
    let tabs = [DiffTab::Schema, DiffTab::Data, DiffTab::Lookups];
    let tab_text = tabs.iter().map(|tab| {
        let is_active = state.diff_review.active_tab == *tab;
        if is_active {
            format!("[{}]", tab.label())
        } else {
            format!(" {} ", tab.label())
        }
    }).collect::<Vec<_>>().join(" | ");

    col![
        Element::styled_text(Line::from(Span::styled(
            "Step 4: Review Sync Plan".to_string(),
            Style::default().fg(theme.accent_primary).bold()
        ))).build() => Length(1),
        Element::text(tab_text) => Length(1),
        spacer!() => Length(1),
    ]
}

/// Render the main diff content
fn render_diff_content(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    // Clone data we need from state.sync_plan to avoid borrow issues
    let (entity_items, entity_count, selected_plan) = {
        let Some(ref plan) = state.sync_plan else {
            return Element::panel(Element::text("No sync plan available"))
                .title("Error")
                .build();
        };

        let items: Vec<EntityPlanItem> = plan.entity_plans.iter().map(|p| {
            EntityPlanItem {
                logical_name: p.entity_info.logical_name.clone(),
                display_name: p.entity_info.display_name.clone(),
                category: p.entity_info.category,
                has_changes: p.schema_diff.has_changes(),
            }
        }).collect();

        let count = plan.entity_plans.len();

        // Get selected plan if any
        let selected_idx = state.diff_review.entity_list.selected();
        let selected = selected_idx.and_then(|idx| plan.entity_plans.get(idx).cloned());

        (items, count, selected)
    };

    let entity_list = Element::list(
        "diff-entity-list",
        &entity_items,
        &state.diff_review.entity_list,
        theme,
    )
    .on_select(Msg::DiffEntityListSelect)
    .on_navigate(Msg::DiffEntityListNavigate)
    .build();

    let entity_panel = Element::panel(entity_list)
        .title(format!("Entities ({})", entity_count))
        .build();

    // Detail panel (right side)
    let detail_panel = if let Some(selected) = selected_plan {
        match state.diff_review.active_tab {
            DiffTab::Schema => render_schema_tab(state, &selected, theme),
            DiffTab::Data => render_data_tab(&selected, theme),
            DiffTab::Lookups => render_lookups_tab(&selected, theme),
        }
    } else {
        Element::panel(Element::text("Select an entity to view details"))
            .title("Details")
            .build()
    };

    row![
        entity_panel => Length(35),
        detail_panel => Fill(1),
    ]
}

/// Render the schema tab content
fn render_schema_tab(state: &mut State, plan: &EntitySyncPlan, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let diff = &plan.schema_diff;

    // Summary line
    let summary_text = format!(
        "Match: {}  Add: {}  Manual: {}",
        diff.fields_in_both.len(),
        diff.fields_to_add.len(),
        diff.fields_target_only.len() + diff.fields_type_mismatch.len()
    );
    let summary = Element::text(summary_text);

    // Build field list - show fields to add and mismatches first, then matching
    let mut field_items: Vec<FieldDiffItem> = Vec::new();

    // Fields to add (non-system only)
    for field in diff.fields_to_add.iter().filter(|f| !f.is_system_field) {
        field_items.push(FieldDiffItem {
            logical_name: field.logical_name.clone(),
            display_name: field.display_name.clone(),
            field_type: field.field_type.clone(),
            status: field.status.clone(),
            is_system_field: field.is_system_field,
        });
    }

    // Type mismatches
    for field in &diff.fields_type_mismatch {
        field_items.push(FieldDiffItem {
            logical_name: field.logical_name.clone(),
            display_name: field.display_name.clone(),
            field_type: field.field_type.clone(),
            status: field.status.clone(),
            is_system_field: field.is_system_field,
        });
    }

    // Target-only (manual review)
    for field in diff.fields_target_only.iter().filter(|f| !f.is_system_field) {
        field_items.push(FieldDiffItem {
            logical_name: field.logical_name.clone(),
            display_name: field.display_name.clone(),
            field_type: field.field_type.clone(),
            status: field.status.clone(),
            is_system_field: field.is_system_field,
        });
    }

    // Matching fields (non-system)
    for field in diff.fields_in_both.iter().filter(|f| !f.is_system_field) {
        field_items.push(FieldDiffItem {
            logical_name: field.logical_name.clone(),
            display_name: field.display_name.clone(),
            field_type: field.field_type.clone(),
            status: field.status.clone(),
            is_system_field: field.is_system_field,
        });
    }

    let field_list = Element::list(
        "diff-field-list",
        &field_items,
        &state.diff_review.field_list,
        theme,
    )
    .on_navigate(Msg::DiffFieldListNavigate)
    .on_render(Msg::DiffSetViewportHeight)
    .build();

    let name = plan.entity_info.display_name
        .as_ref()
        .unwrap_or(&plan.entity_info.logical_name);

    col![
        summary => Length(1),
        spacer!() => Length(1),
        Element::panel(field_list).title(format!("{} - Schema", name)).build() => Fill(1),
    ]
}

/// Render the data tab content
fn render_data_tab(plan: &EntitySyncPlan, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let preview = &plan.data_preview;
    let name = plan.entity_info.display_name
        .as_ref()
        .unwrap_or(&plan.entity_info.logical_name);

    let origin_text = format!("Records in origin: {}", preview.origin_count);
    let target_text = format!("Records in target: {}", preview.target_count);
    let delete_text = format!("  1. Delete {} records from target", preview.target_count);
    let copy_text = format!("  2. Copy {} records from origin (preserving GUIDs)", preview.origin_count);

    let empty_note = if preview.origin_count == 0 && preview.target_count == 0 {
        Element::text("ℹ Both environments have no records for this entity")
    } else {
        Element::text("")
    };

    let content = Element::column(vec![
        Element::text(origin_text),
        Element::text(target_text),
        spacer!(),
        Element::text("Operation:"),
        Element::text(delete_text),
        Element::text(copy_text),
        spacer!(),
        empty_note,
    ]).build();

    Element::panel(content)
        .title(format!("{} - Data Preview", name))
        .build()
}

/// Render the lookups tab content
fn render_lookups_tab(plan: &EntitySyncPlan, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let name = plan.entity_info.display_name
        .as_ref()
        .unwrap_or(&plan.entity_info.logical_name);

    let mut lines: Vec<Element<Msg>> = vec![];

    // Internal lookups (to other selected entities)
    let internal: Vec<_> = plan.entity_info.lookups.iter()
        .filter(|l| l.is_internal)
        .collect();

    if !internal.is_empty() {
        lines.push(Element::text("Internal Lookups (preserved):"));

        for lookup in &internal {
            let text = format!("  ✓ {} → {}", lookup.field_name, lookup.target_entity);
            lines.push(Element::text(text));
        }

        lines.push(Element::text(""));
    }

    // External lookups (to entities not in selection - will be nulled)
    let external: Vec<_> = plan.entity_info.lookups.iter()
        .filter(|l| !l.is_internal)
        .collect();

    if !external.is_empty() {
        lines.push(Element::text("External Lookups (will be nulled):"));

        for lookup in &external {
            let text = format!("  ⚠ {} → {} (not in sync set)", lookup.field_name, lookup.target_entity);
            lines.push(Element::text(text));
        }

        lines.push(Element::text(""));
    }

    // Nulled lookups with affected counts
    if !plan.nulled_lookups.is_empty() {
        lines.push(Element::text("Affected Records:"));

        for nulled in &plan.nulled_lookups {
            let text = format!("  ✗ {} ({} records)", nulled.field_name, nulled.affected_count);
            lines.push(Element::text(text));
        }
    }

    // No lookups message
    if internal.is_empty() && external.is_empty() {
        lines.push(Element::text("ℹ No lookup fields on this entity"));
    }

    let content = Element::column(lines).build();

    Element::panel(content)
        .title(format!("{} - Lookups", name))
        .build()
}

/// Render step footer with navigation
fn render_step_footer(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    // Summary stats
    let stats = if let Some(ref plan) = state.sync_plan {
        let schema_changes = plan.entity_plans.iter()
            .filter(|p| p.schema_diff.has_changes())
            .count();

        let text = format!(
            "Tab Switch tabs | {} entities, {} with schema changes",
            plan.entity_plans.len(),
            schema_changes
        );
        Element::text(text)
    } else {
        Element::text("")
    };

    let buttons = button_row![
        ("diff-back-btn", "Back (Esc)", Msg::Back),
        ("diff-next-btn", "Confirm (Enter)", Msg::Next),
    ];

    col![
        stats => Length(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ]
}
