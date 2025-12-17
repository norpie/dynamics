use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::api::FieldMetadata;
use crate::transfer::ResolverFallback;
use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::modals::ConfirmationModal;
use crate::tui::resource::Resource;
use crate::tui::widgets::events::TreeEvent;
use crate::tui::{Alignment, Element, LayeredView, LayoutConstraint, Subscription, Theme};

use super::state::{DeleteTarget, Msg, State, TransformType};
use super::tree::build_tree;

pub fn render(state: &mut State, theme: &Theme) -> LayeredView<Msg> {
    // Extract config state to avoid borrow issues
    let (is_loading, is_error, error_msg, source_env, target_env) = match &state.config {
        Resource::NotAsked | Resource::Loading => (true, false, None, String::new(), String::new()),
        Resource::Failure(err) => (false, true, Some(err.clone()), String::new(), String::new()),
        Resource::Success(config) => (false, false, None, config.source_env.clone(), config.target_env.clone()),
    };

    let content = if is_loading {
        Element::text("Loading config...")
    } else if is_error {
        let err = error_msg.unwrap_or_default();
        Element::styled_text(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(theme.accent_error)),
            Span::styled(err, Style::default().fg(theme.text_primary)),
        ]))
        .build()
    } else {
        render_editor(state, &source_env, &target_env, theme)
    };

    let main_view = Element::panel(content).title(&state.config_name).build();
    let mut view = LayeredView::new(main_view);

    // Entity modal
    if state.show_entity_modal {
        let source_entities = match &state.source_entities {
            Resource::Success(e) => e.as_slice(),
            _ => &[],
        };
        let target_entities = match &state.target_entities {
            Resource::Success(e) => e.as_slice(),
            _ => &[],
        };
        view = view.with_app_modal(
            render_entity_modal(
                &mut state.entity_form,
                state.editing_entity_idx.is_some(),
                source_entities,
                target_entities,
                theme,
            ),
            Alignment::Center,
        );
    }

    // Field modal
    if state.show_field_modal {
        let source_fields = match &state.source_fields {
            Resource::Success(f) => f.as_slice(),
            _ => &[],
        };
        let target_fields = match &state.target_fields {
            Resource::Success(f) => f.as_slice(),
            _ => &[],
        };
        let fields_loading = matches!(&state.source_fields, Resource::Loading)
            || matches!(&state.target_fields, Resource::Loading);
        let resolvers: Vec<_> = match &state.config {
            Resource::Success(config) => config.resolvers.iter().map(|r| {
                let match_field = r.match_fields.first().map(|mf| mf.target_field.as_str()).unwrap_or("");
                (r.name.as_str(), r.source_entity.as_str(), match_field)
            }).collect(),
            _ => vec![],
        };

        view = view.with_app_modal(
            render_field_modal(
                &mut state.field_form,
                state.editing_field.map(|(_, f)| f != usize::MAX).unwrap_or(false),
                source_fields,
                target_fields,
                fields_loading,
                &resolvers,
                theme,
            ),
            Alignment::Center,
        );
    }

    // Resolver modal
    if state.show_resolver_modal {
        let target_entities = match &state.target_entities {
            Resource::Success(e) => e.as_slice(),
            _ => &[],
        };
        let match_fields = match &state.resolver_match_fields {
            Resource::Success(f) => f.as_slice(),
            _ => &[],
        };
        let fields_loading = matches!(&state.resolver_match_fields, Resource::Loading);

        view = view.with_app_modal(
            render_resolver_modal(
                &mut state.resolver_form,
                state.editing_resolver_idx.is_some(),
                target_entities,
                match_fields,
                fields_loading,
                theme,
            ),
            Alignment::Center,
        );
    }

    // Delete confirmation
    if state.show_delete_confirm {
        let message = match &state.delete_target {
            Some(DeleteTarget::Entity(_)) => "Delete this entity mapping and all its field mappings?",
            Some(DeleteTarget::Field(_, _)) => "Delete this field mapping?",
            Some(DeleteTarget::Resolver(_)) => "Delete this resolver?",
            None => "Delete?",
        };

        let modal = ConfirmationModal::new("Confirm Delete")
            .message(message)
            .confirm_text("Delete")
            .cancel_text("Cancel")
            .on_confirm(Msg::ConfirmDelete)
            .on_cancel(Msg::CancelDelete)
            .build();

        view = view.with_app_modal(modal, Alignment::Center);
    }

    view
}

fn render_editor(state: &mut State, source_env: &str, target_env: &str, theme: &Theme) -> Element<Msg> {
    let items = match &state.config {
        Resource::Success(config) => build_tree(config),
        _ => vec![],
    };

    let tree = if items.is_empty() {
        Element::styled_text(Line::from(vec![
            Span::styled(
                "No entity mappings. Press 'a' to add one.",
                Style::default().fg(theme.text_secondary),
            ),
        ]))
        .build()
    } else {
        Element::tree(FocusId::new("mapping-tree"), &items, &mut state.tree_state, theme)
            .on_event(Msg::TreeEvent)
            .on_select(Msg::TreeSelect)
            .build()
    };

    let tree_panel = Element::panel(tree).title("Entity Mappings").build();

    // Header info
    let header = Element::styled_text(Line::from(vec![
        Span::styled("Source: ", Style::default().fg(theme.text_tertiary)),
        Span::styled(source_env.to_string(), Style::default().fg(theme.accent_primary)),
        Span::styled(" → Target: ", Style::default().fg(theme.text_tertiary)),
        Span::styled(target_env.to_string(), Style::default().fg(theme.accent_secondary)),
    ]))
    .build();

    // Button row
    let back_btn = Element::button(FocusId::new("back-btn"), "Back")
        .on_press(Msg::Back)
        .build();

    let preview_btn = Element::button(FocusId::new("preview-btn"), "Preview")
        .on_press(Msg::Preview)
        .build();

    let button_row = RowBuilder::new()
        .add(back_btn, LayoutConstraint::Length(10))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(preview_btn, LayoutConstraint::Length(12))
        .build();

    ColumnBuilder::new()
        .add(header, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(tree_panel, LayoutConstraint::Fill(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(button_row, LayoutConstraint::Length(3))
        .build()
}

fn render_entity_modal(
    form: &mut super::state::EntityMappingForm,
    is_edit: bool,
    source_entities: &[String],
    target_entities: &[String],
    theme: &Theme,
) -> Element<Msg> {
    let title = if is_edit { "Edit Entity Mapping" } else { "Add Entity Mapping" };

    // Source entity autocomplete
    let source_input = Element::autocomplete(
        FocusId::new("entity-source"),
        source_entities.to_vec(),
        form.source_entity.value.clone(),
        &mut form.source_entity.state,
    )
    .placeholder("Type to search source entities...")
    .on_event(Msg::EntityFormSource)
    .build();
    let source_panel = Element::panel(source_input).title("Source Entity").build();

    // Target entity autocomplete
    let target_input = Element::autocomplete(
        FocusId::new("entity-target"),
        target_entities.to_vec(),
        form.target_entity.value.clone(),
        &mut form.target_entity.state,
    )
    .placeholder("Type to search target entities...")
    .on_event(Msg::EntityFormTarget)
    .build();
    let target_panel = Element::panel(target_input).title("Target Entity").build();

    // Priority input
    let priority_input = Element::text_input(
        FocusId::new("entity-priority"),
        &form.priority.value,
        &mut form.priority.state,
    )
    .placeholder("1")
    .on_event(Msg::EntityFormPriority)
    .build();
    let priority_panel = Element::panel(priority_input).title("Priority (lower = first)").build();

    // Operation filter toggles
    let creates_label = if form.allow_creates { "[x] Creates" } else { "[ ] Creates" };
    let creates_btn = Element::button(FocusId::new("entity-creates"), creates_label)
        .on_press(Msg::EntityFormToggleCreates)
        .build();

    let updates_label = if form.allow_updates { "[x] Updates" } else { "[ ] Updates" };
    let updates_btn = Element::button(FocusId::new("entity-updates"), updates_label)
        .on_press(Msg::EntityFormToggleUpdates)
        .build();

    let deletes_label = if form.allow_deletes { "[x] Deletes" } else { "[ ] Deletes" };
    let deletes_btn = Element::button(FocusId::new("entity-deletes"), deletes_label)
        .on_press(Msg::EntityFormToggleDeletes)
        .build();

    let deactivates_label = if form.allow_deactivates { "[x] Deactivates" } else { "[ ] Deactivates" };
    let deactivates_btn = Element::button(FocusId::new("entity-deactivates"), deactivates_label)
        .on_press(Msg::EntityFormToggleDeactivates)
        .build();

    // Operation filter row
    let op_filter_row = RowBuilder::new()
        .add(creates_btn, LayoutConstraint::Length(14))
        .add(updates_btn, LayoutConstraint::Length(14))
        .add(deletes_btn, LayoutConstraint::Length(14))
        .add(deactivates_btn, LayoutConstraint::Length(16))
        .spacing(1)
        .build();

    let op_filter_panel = Element::panel(op_filter_row).title("Operations").build();

    // Buttons
    let cancel_btn = Element::button(FocusId::new("entity-cancel"), "Cancel")
        .on_press(Msg::CloseEntityModal)
        .build();

    let save_btn = if form.is_valid() {
        Element::button(FocusId::new("entity-save"), "Save")
            .on_press(Msg::SaveEntity)
            .build()
    } else {
        Element::button(FocusId::new("entity-save"), "Save").build()
    };

    let button_row = RowBuilder::new()
        .add(cancel_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(save_btn, LayoutConstraint::Length(12))
        .build();

    let form_content = ColumnBuilder::new()
        .add(source_panel, LayoutConstraint::Length(3))
        .add(target_panel, LayoutConstraint::Length(3))
        .add(priority_panel, LayoutConstraint::Length(3))
        .add(op_filter_panel, LayoutConstraint::Length(5))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    Element::panel(Element::container(form_content).padding(1).build())
        .title(title)
        .width(65)
        .height(26)
        .build()
}

fn render_field_modal(
    form: &mut super::state::FieldMappingForm,
    is_edit: bool,
    source_fields: &[FieldMetadata],
    target_fields: &[FieldMetadata],
    fields_loading: bool,
    resolvers: &[(&str, &str, &str)], // (name, source_entity, match_field)
    theme: &Theme,
) -> Element<Msg> {
    use super::state::{ConditionType, FallbackType};

    let title = if is_edit { "Edit Field Mapping" } else { "Add Field Mapping" };

    // Target field autocomplete
    let target_options: Vec<String> = target_fields.iter().map(|f| f.logical_name.clone()).collect();
    let target_input = Element::autocomplete(
        FocusId::new("field-target"),
        target_options,
        form.target_field.value.clone(),
        &mut form.target_field.state,
    )
    .placeholder(if fields_loading { "Loading fields..." } else { "Type to search target fields..." })
    .on_event(Msg::FieldFormTarget)
    .build();
    let target_panel = Element::panel(target_input).title("Target Field").build();

    // Transform type indicator
    let type_label = format!("{} (Ctrl+T to cycle)", form.transform_type.label());
    let type_indicator = Element::styled_text(Line::from(vec![
        Span::styled("Transform: ", Style::default().fg(theme.text_tertiary)),
        Span::styled(type_label, Style::default().fg(theme.accent_primary)),
    ]))
    .build();

    let source_options: Vec<String> = source_fields.iter().map(|f| f.logical_name.clone()).collect();

    // Build transform-specific form section
    let (transform_content, modal_height) = match form.transform_type {
        TransformType::Copy => {
            let input = Element::autocomplete(
                FocusId::new("field-source"),
                source_options,
                form.source_path.value.clone(),
                &mut form.source_path.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "e.g., name or accountid.name" })
            .on_event(Msg::FieldFormSourcePath)
            .build();
            let source_panel = Element::panel(input).title("Source Field").build();

            // Resolver button (cycle through available resolvers)
            let resolver_label = if let Some(ref name) = form.resolver_name {
                // Find the resolver details
                if let Some((_, entity, field)) = resolvers.iter().find(|(n, _, _)| *n == name) {
                    format!("Resolver: {} → {}.{}", name, entity, field)
                } else {
                    format!("Resolver: {} (not found)", name)
                }
            } else if resolvers.is_empty() {
                "Resolver: (none available)".to_string()
            } else {
                format!("Resolver: (none) - Ctrl+R to cycle ({} available)", resolvers.len())
            };

            let resolver_btn = if !resolvers.is_empty() {
                Element::button(FocusId::new("field-resolver"), &resolver_label)
                    .on_press(Msg::FieldFormCycleResolver)
                    .build()
            } else {
                Element::styled_text(Line::from(vec![
                    Span::styled(resolver_label, Style::default().fg(theme.text_tertiary)),
                ]))
                .build()
            };

            let content = ColumnBuilder::new()
                .add(source_panel, LayoutConstraint::Length(3))
                .add(resolver_btn, LayoutConstraint::Length(3))
                .spacing(1)
                .build();

            (content, 22)
        }
        TransformType::Constant => {
            let input = Element::text_input(
                FocusId::new("field-constant"),
                &form.constant_value.value,
                &mut form.constant_value.state,
            )
            .placeholder("e.g., true, 42, or string (empty = null)")
            .on_event(Msg::FieldFormConstant)
            .build();
            (Element::panel(input).title("Constant Value").build(), 18)
        }
        TransformType::Conditional => {
            // Source field
            let source_input = Element::autocomplete(
                FocusId::new("cond-source"),
                source_options,
                form.condition_source.value.clone(),
                &mut form.condition_source.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "Source field to check" })
            .on_event(Msg::FieldFormConditionSource)
            .build();
            let source_panel = Element::panel(source_input).title("Source Field").build();

            // Condition type indicator
            let cond_label = format!("{} (Ctrl+C to cycle)", form.condition_type.label());
            let cond_indicator = Element::styled_text(Line::from(vec![
                Span::styled("Condition: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(cond_label, Style::default().fg(theme.accent_secondary)),
            ])).build();

            // Condition value (only for equals/not equals)
            let cond_value_panel = if form.condition_type.needs_value() {
                let value_input = Element::text_input(
                    FocusId::new("cond-value"),
                    &form.condition_value.value,
                    &mut form.condition_value.state,
                )
                .placeholder("Compare value")
                .on_event(Msg::FieldFormConditionValue)
                .build();
                Some(Element::panel(value_input).title("Compare To").build())
            } else {
                None
            };

            // Then/else values
            let then_input = Element::text_input(
                FocusId::new("cond-then"),
                &form.then_value.value,
                &mut form.then_value.state,
            )
            .placeholder("Value if true")
            .on_event(Msg::FieldFormThenValue)
            .build();
            let then_panel = Element::panel(then_input).title("Then Value").build();

            let else_input = Element::text_input(
                FocusId::new("cond-else"),
                &form.else_value.value,
                &mut form.else_value.state,
            )
            .placeholder("Value if false")
            .on_event(Msg::FieldFormElseValue)
            .build();
            let else_panel = Element::panel(else_input).title("Else Value").build();

            let values_row = RowBuilder::new()
                .add(then_panel, LayoutConstraint::Fill(1))
                .add(Element::text(""), LayoutConstraint::Length(1))
                .add(else_panel, LayoutConstraint::Fill(1))
                .build();

            let mut col = ColumnBuilder::new()
                .add(source_panel, LayoutConstraint::Length(3))
                .add(cond_indicator, LayoutConstraint::Length(1));

            if let Some(value_panel) = cond_value_panel {
                col = col.add(value_panel, LayoutConstraint::Length(3));
            }

            let content = col
                .add(values_row, LayoutConstraint::Length(3))
                .spacing(1)
                .build();

            let height = if form.condition_type.needs_value() { 24 } else { 20 };
            (content, height)
        }
        TransformType::ValueMap => {
            // Source field
            let source_input = Element::autocomplete(
                FocusId::new("map-source"),
                source_options,
                form.value_map_source.value.clone(),
                &mut form.value_map_source.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "Source field to map" })
            .on_event(Msg::FieldFormValueMapSource)
            .build();
            let source_panel = Element::panel(source_input).title("Source Field").build();

            // Fallback indicator
            let fallback_label = format!("{} (Ctrl+F to cycle)", form.value_map_fallback.label());
            let fallback_indicator = Element::styled_text(Line::from(vec![
                Span::styled("Fallback: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(fallback_label, Style::default().fg(theme.accent_secondary)),
            ])).build();

            // Default value panel (only for Default fallback)
            let default_panel = if form.value_map_fallback == FallbackType::Default {
                let default_input = Element::text_input(
                    FocusId::new("map-default"),
                    &form.value_map_default.value,
                    &mut form.value_map_default.state,
                )
                .placeholder("Default value when no mapping matches")
                .on_event(Msg::FieldFormValueMapDefault)
                .build();
                Some(Element::panel(default_input).title("Default Value").build())
            } else {
                None
            };

            // Mappings header + add button
            let add_btn = Element::button(FocusId::new("map-add"), "+ Add")
                .on_press(Msg::FieldFormAddMapping)
                .build();
            let mappings_header = RowBuilder::new()
                .add(Element::styled_text(Line::from(vec![
                    Span::styled("Mappings:", Style::default().fg(theme.text_secondary)),
                ])).build(), LayoutConstraint::Fill(1))
                .add(add_btn, LayoutConstraint::Length(10))
                .build();

            // Mapping entries (show up to 4 in view)
            // Note: Using individual event handlers to avoid closure capture issues
            let mut mappings_col = ColumnBuilder::new();
            let entries_len = form.value_map_entries.len();

            // Entry 0
            if entries_len > 0 {
                let entry = &mut form.value_map_entries[0];
                let src_input = Element::text_input(
                    FocusId::new("map-src-0"),
                    &entry.source_value.value,
                    &mut entry.source_value.state,
                )
                .placeholder("Source")
                .on_event(|e| Msg::FieldFormMappingSource(0, e))
                .build();

                let tgt_input = Element::text_input(
                    FocusId::new("map-tgt-0"),
                    &entry.target_value.value,
                    &mut entry.target_value.state,
                )
                .placeholder("Target")
                .on_event(|e| Msg::FieldFormMappingTarget(0, e))
                .build();

                let del_btn = Element::button(FocusId::new("map-del-0"), "×")
                    .on_press(Msg::FieldFormRemoveMapping(0))
                    .build();

                let entry_row = RowBuilder::new()
                    .add(src_input, LayoutConstraint::Fill(1))
                    .add(Element::text(" → "), LayoutConstraint::Length(4))
                    .add(tgt_input, LayoutConstraint::Fill(1))
                    .add(del_btn, LayoutConstraint::Length(5))
                    .build();
                mappings_col = mappings_col.add(entry_row, LayoutConstraint::Length(3));
            }

            // Entry 1
            if entries_len > 1 {
                let entry = &mut form.value_map_entries[1];
                let src_input = Element::text_input(
                    FocusId::new("map-src-1"),
                    &entry.source_value.value,
                    &mut entry.source_value.state,
                )
                .placeholder("Source")
                .on_event(|e| Msg::FieldFormMappingSource(1, e))
                .build();

                let tgt_input = Element::text_input(
                    FocusId::new("map-tgt-1"),
                    &entry.target_value.value,
                    &mut entry.target_value.state,
                )
                .placeholder("Target")
                .on_event(|e| Msg::FieldFormMappingTarget(1, e))
                .build();

                let del_btn = Element::button(FocusId::new("map-del-1"), "×")
                    .on_press(Msg::FieldFormRemoveMapping(1))
                    .build();

                let entry_row = RowBuilder::new()
                    .add(src_input, LayoutConstraint::Fill(1))
                    .add(Element::text(" → "), LayoutConstraint::Length(4))
                    .add(tgt_input, LayoutConstraint::Fill(1))
                    .add(del_btn, LayoutConstraint::Length(5))
                    .build();
                mappings_col = mappings_col.add(entry_row, LayoutConstraint::Length(3));
            }

            // Entry 2
            if entries_len > 2 {
                let entry = &mut form.value_map_entries[2];
                let src_input = Element::text_input(
                    FocusId::new("map-src-2"),
                    &entry.source_value.value,
                    &mut entry.source_value.state,
                )
                .placeholder("Source")
                .on_event(|e| Msg::FieldFormMappingSource(2, e))
                .build();

                let tgt_input = Element::text_input(
                    FocusId::new("map-tgt-2"),
                    &entry.target_value.value,
                    &mut entry.target_value.state,
                )
                .placeholder("Target")
                .on_event(|e| Msg::FieldFormMappingTarget(2, e))
                .build();

                let del_btn = Element::button(FocusId::new("map-del-2"), "×")
                    .on_press(Msg::FieldFormRemoveMapping(2))
                    .build();

                let entry_row = RowBuilder::new()
                    .add(src_input, LayoutConstraint::Fill(1))
                    .add(Element::text(" → "), LayoutConstraint::Length(4))
                    .add(tgt_input, LayoutConstraint::Fill(1))
                    .add(del_btn, LayoutConstraint::Length(5))
                    .build();
                mappings_col = mappings_col.add(entry_row, LayoutConstraint::Length(3));
            }

            // Entry 3
            if entries_len > 3 {
                let entry = &mut form.value_map_entries[3];
                let src_input = Element::text_input(
                    FocusId::new("map-src-3"),
                    &entry.source_value.value,
                    &mut entry.source_value.state,
                )
                .placeholder("Source")
                .on_event(|e| Msg::FieldFormMappingSource(3, e))
                .build();

                let tgt_input = Element::text_input(
                    FocusId::new("map-tgt-3"),
                    &entry.target_value.value,
                    &mut entry.target_value.state,
                )
                .placeholder("Target")
                .on_event(|e| Msg::FieldFormMappingTarget(3, e))
                .build();

                let del_btn = Element::button(FocusId::new("map-del-3"), "×")
                    .on_press(Msg::FieldFormRemoveMapping(3))
                    .build();

                let entry_row = RowBuilder::new()
                    .add(src_input, LayoutConstraint::Fill(1))
                    .add(Element::text(" → "), LayoutConstraint::Length(4))
                    .add(tgt_input, LayoutConstraint::Fill(1))
                    .add(del_btn, LayoutConstraint::Length(5))
                    .build();
                mappings_col = mappings_col.add(entry_row, LayoutConstraint::Length(3));
            }

            if form.value_map_entries.len() > 4 {
                let more_text = format!("... and {} more", form.value_map_entries.len() - 4);
                mappings_col = mappings_col.add(
                    Element::styled_text(Line::from(vec![
                        Span::styled(more_text, Style::default().fg(theme.text_tertiary)),
                    ])).build(),
                    LayoutConstraint::Length(1),
                );
            }

            let mut col = ColumnBuilder::new()
                .add(source_panel, LayoutConstraint::Length(3))
                .add(fallback_indicator, LayoutConstraint::Length(1));

            if let Some(panel) = default_panel {
                col = col.add(panel, LayoutConstraint::Length(3));
            }

            let content = col
                .add(mappings_header, LayoutConstraint::Length(3))
                .add(mappings_col.build(), LayoutConstraint::Fill(1))
                .spacing(1)
                .build();

            // Height calculation:
            // - Border + padding: 4
            // - Target field panel: 3 + spacing 1 = 4
            // - Type indicator: 1 + spacing 1 = 2
            // - Source panel: 3 + spacing 1 = 4
            // - Fallback indicator: 1 + spacing 1 = 2
            // - Default panel (optional): 3 + spacing 1 = 4
            // - Mappings header: 3 + spacing 1 = 4
            // - Entries: 3 each
            // - Button row: 3
            let base_height: u16 = if form.value_map_fallback == FallbackType::Default { 32 } else { 28 };
            let entries_height = (form.value_map_entries.len().min(4) * 4) as u16;
            let height = base_height + entries_height;
            (content, height.min(45))
        }
        TransformType::Format => {
            // Template input
            let template_input = Element::text_input(
                FocusId::new("format-template"),
                &form.format_template.value,
                &mut form.format_template.state,
            )
            .placeholder(r#"e.g., ${firstname} ${lastname} or ${price:,.2f}"#)
            .on_event(Msg::FieldFormFormatTemplate)
            .build();
            let template_panel = Element::panel(template_input).title("Template").build();

            // Null handling indicator
            let null_label = format!("{} (Ctrl+N to cycle)", form.format_null_handling.label());
            let null_indicator = Element::styled_text(Line::from(vec![
                Span::styled("Null Handling: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(null_label, Style::default().fg(theme.accent_secondary)),
            ])).build();

            // Help text
            let help_text = Element::styled_text(Line::from(vec![
                Span::styled("Syntax: ", Style::default().fg(theme.text_tertiary)),
                Span::raw("${field}, ${a + b}, ${cond ? then : else}, ${a ?? b}, ${val:,.2f}"),
            ])).build();

            let content = ColumnBuilder::new()
                .add(template_panel, LayoutConstraint::Length(3))
                .add(null_indicator, LayoutConstraint::Length(1))
                .add(help_text, LayoutConstraint::Length(1))
                .spacing(1)
                .build();

            (content, 20)
        }
    };

    // Run validation
    let validation = form.validate(target_fields, source_fields);
    let can_save = form.is_valid() && !validation.has_errors();

    // Validation message (show error or warning)
    let validation_msg = if let Some(err) = validation.first_error() {
        Some(Element::styled_text(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(theme.accent_error)),
            Span::styled(err.to_string(), Style::default().fg(theme.accent_error)),
        ])).build())
    } else if let Some(warn) = validation.first_warning() {
        Some(Element::styled_text(Line::from(vec![
            Span::styled("Warning: ", Style::default().fg(theme.accent_warning)),
            Span::styled(warn.to_string(), Style::default().fg(theme.text_secondary)),
        ])).build())
    } else {
        None
    };

    // Buttons
    let cancel_btn = Element::button(FocusId::new("field-cancel"), "Cancel")
        .on_press(Msg::CloseFieldModal)
        .build();

    let save_btn = if can_save {
        Element::button(FocusId::new("field-save"), "Save")
            .on_press(Msg::SaveField)
            .build()
    } else {
        Element::button(FocusId::new("field-save"), "Save").build()
    };

    let button_row = RowBuilder::new()
        .add(cancel_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(save_btn, LayoutConstraint::Length(12))
        .build();

    let mut form_builder = ColumnBuilder::new()
        .add(target_panel, LayoutConstraint::Length(3))
        .add(type_indicator, LayoutConstraint::Length(1))
        .add(transform_content, LayoutConstraint::Fill(1));

    if let Some(msg) = validation_msg {
        form_builder = form_builder.add(msg, LayoutConstraint::Length(1));
    }

    let form_content = form_builder
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    // Adjust height for validation message
    let final_height = if validation.has_errors() || validation.has_warnings() {
        modal_height + 2
    } else {
        modal_height
    };

    Element::panel(Element::container(form_content).padding(1).build())
        .title(title)
        .width(70)
        .height(final_height)
        .build()
}

fn render_resolver_modal(
    form: &mut super::state::ResolverForm,
    is_edit: bool,
    target_entities: &[String],
    match_fields: &[FieldMetadata],
    fields_loading: bool,
    theme: &Theme,
) -> Element<Msg> {
    let title = if is_edit { "Edit Resolver" } else { "Add Resolver" };

    // Name input
    let name_input = Element::text_input(
        FocusId::new("resolver-name"),
        &form.name.value,
        &mut form.name.state,
    )
    .placeholder("e.g., contact_by_email")
    .on_event(Msg::ResolverFormName)
    .build();
    let name_panel = Element::panel(name_input).title("Resolver Name").build();

    // Source entity autocomplete (searches in target environment)
    let entity_input = Element::autocomplete(
        FocusId::new("resolver-entity"),
        target_entities.to_vec(),
        form.source_entity.value.clone(),
        &mut form.source_entity.state,
    )
    .placeholder("Type to search entities...")
    .on_event(Msg::ResolverFormSourceEntity)
    .build();
    let entity_panel = Element::panel(entity_input).title("Source Entity (to search in target)").build();

    // Match field autocomplete (single field - the original behavior)
    // For compound keys, user adds more fields with "+ Add Field"
    let match_options: Vec<String> = match_fields.iter().map(|f| f.logical_name.clone()).collect();
    let rows_len = form.match_field_rows.len();

    // For single field, show simple input; for multiple, show list with add/remove
    let match_field_content = if rows_len == 1 {
        // Single field - simple autocomplete like before
        let row = &mut form.match_field_rows[0];
        let match_input = Element::autocomplete(
            FocusId::new("resolver-field-0"),
            match_options.clone(),
            row.target_field.value.clone(),
            &mut row.target_field.state,
        )
        .placeholder(if fields_loading { "Loading fields..." } else { "Field to match against" })
        .on_event(|e| Msg::ResolverMatchField(0, e))
        .build();

        let add_btn = Element::button(FocusId::new("resolver-add-row"), "+ Compound Key")
            .on_press(Msg::ResolverAddMatchFieldRow)
            .build();

        let row_with_add = RowBuilder::new()
            .add(match_input, LayoutConstraint::Fill(1))
            .add(Element::text(" "), LayoutConstraint::Length(1))
            .add(add_btn, LayoutConstraint::Length(16))
            .build();

        Element::panel(row_with_add).title("Match Field").build()
    } else {
        // Multiple fields - show list with source_path → target_field
        let mut match_rows_col = ColumnBuilder::new();

        // Header row
        let header = RowBuilder::new()
            .add(Element::text("   "), LayoutConstraint::Length(3))
            .add(Element::styled_text(Line::from(Span::styled("Source Path", Style::default().fg(theme.text_secondary)))).build(), LayoutConstraint::Fill(1))
            .add(Element::text("  "), LayoutConstraint::Length(2))
            .add(Element::styled_text(Line::from(Span::styled("Target Field", Style::default().fg(theme.text_secondary)))).build(), LayoutConstraint::Fill(1))
            .add(Element::text("     "), LayoutConstraint::Length(5))
            .build();
        match_rows_col = match_rows_col.add(header, LayoutConstraint::Length(1));

        // Row 0
        if rows_len > 0 {
            let row = &mut form.match_field_rows[0];
            let source_input = Element::text_input(
                FocusId::new("resolver-source-0"),
                &row.source_path.value,
                &mut row.source_path.state,
            )
            .placeholder("Source field path")
            .on_event(|e| Msg::ResolverSourcePath(0, e))
            .build();

            let target_input = Element::autocomplete(
                FocusId::new("resolver-field-0"),
                match_options.clone(),
                row.target_field.value.clone(),
                &mut row.target_field.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "Target field" })
            .on_event(|e| Msg::ResolverMatchField(0, e))
            .build();

            let del_btn = Element::button(FocusId::new("resolver-del-0"), "×")
                .on_press(Msg::ResolverRemoveMatchFieldRow)
                .build();

            let entry_row = RowBuilder::new()
                .add(Element::text("1. "), LayoutConstraint::Length(3))
                .add(source_input, LayoutConstraint::Fill(1))
                .add(Element::text("→ "), LayoutConstraint::Length(2))
                .add(target_input, LayoutConstraint::Fill(1))
                .add(del_btn, LayoutConstraint::Length(5))
                .build();
            match_rows_col = match_rows_col.add(entry_row, LayoutConstraint::Length(3));
        }

        // Row 1
        if rows_len > 1 {
            let row = &mut form.match_field_rows[1];
            let source_input = Element::text_input(
                FocusId::new("resolver-source-1"),
                &row.source_path.value,
                &mut row.source_path.state,
            )
            .placeholder("Source field path")
            .on_event(|e| Msg::ResolverSourcePath(1, e))
            .build();

            let target_input = Element::autocomplete(
                FocusId::new("resolver-field-1"),
                match_options.clone(),
                row.target_field.value.clone(),
                &mut row.target_field.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "Target field" })
            .on_event(|e| Msg::ResolverMatchField(1, e))
            .build();

            let del_btn = Element::button(FocusId::new("resolver-del-1"), "×")
                .on_press(Msg::ResolverRemoveMatchFieldRow)
                .build();

            let entry_row = RowBuilder::new()
                .add(Element::text("2. "), LayoutConstraint::Length(3))
                .add(source_input, LayoutConstraint::Fill(1))
                .add(Element::text("→ "), LayoutConstraint::Length(2))
                .add(target_input, LayoutConstraint::Fill(1))
                .add(del_btn, LayoutConstraint::Length(5))
                .build();
            match_rows_col = match_rows_col.add(entry_row, LayoutConstraint::Length(3));
        }

        // Row 2
        if rows_len > 2 {
            let row = &mut form.match_field_rows[2];
            let source_input = Element::text_input(
                FocusId::new("resolver-source-2"),
                &row.source_path.value,
                &mut row.source_path.state,
            )
            .placeholder("Source field path")
            .on_event(|e| Msg::ResolverSourcePath(2, e))
            .build();

            let target_input = Element::autocomplete(
                FocusId::new("resolver-field-2"),
                match_options.clone(),
                row.target_field.value.clone(),
                &mut row.target_field.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "Target field" })
            .on_event(|e| Msg::ResolverMatchField(2, e))
            .build();

            let del_btn = Element::button(FocusId::new("resolver-del-2"), "×")
                .on_press(Msg::ResolverRemoveMatchFieldRow)
                .build();

            let entry_row = RowBuilder::new()
                .add(Element::text("3. "), LayoutConstraint::Length(3))
                .add(source_input, LayoutConstraint::Fill(1))
                .add(Element::text("→ "), LayoutConstraint::Length(2))
                .add(target_input, LayoutConstraint::Fill(1))
                .add(del_btn, LayoutConstraint::Length(5))
                .build();
            match_rows_col = match_rows_col.add(entry_row, LayoutConstraint::Length(3));
        }

        // Row 3
        if rows_len > 3 {
            let row = &mut form.match_field_rows[3];
            let source_input = Element::text_input(
                FocusId::new("resolver-source-3"),
                &row.source_path.value,
                &mut row.source_path.state,
            )
            .placeholder("Source field path")
            .on_event(|e| Msg::ResolverSourcePath(3, e))
            .build();

            let target_input = Element::autocomplete(
                FocusId::new("resolver-field-3"),
                match_options.clone(),
                row.target_field.value.clone(),
                &mut row.target_field.state,
            )
            .placeholder(if fields_loading { "Loading..." } else { "Target field" })
            .on_event(|e| Msg::ResolverMatchField(3, e))
            .build();

            let del_btn = Element::button(FocusId::new("resolver-del-3"), "×")
                .on_press(Msg::ResolverRemoveMatchFieldRow)
                .build();

            let entry_row = RowBuilder::new()
                .add(Element::text("4. "), LayoutConstraint::Length(3))
                .add(source_input, LayoutConstraint::Fill(1))
                .add(Element::text("→ "), LayoutConstraint::Length(2))
                .add(target_input, LayoutConstraint::Fill(1))
                .add(del_btn, LayoutConstraint::Length(5))
                .build();
            match_rows_col = match_rows_col.add(entry_row, LayoutConstraint::Length(3));
        }

        if rows_len > 4 {
            let more_text = format!("... and {} more", rows_len - 4);
            match_rows_col = match_rows_col.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(more_text, Style::default().fg(theme.text_tertiary)),
                ]))
                .build(),
                LayoutConstraint::Length(1),
            );
        }

        // Add button at bottom
        let add_btn = Element::button(FocusId::new("resolver-add-row"), "+ Add Field")
            .on_press(Msg::ResolverAddMatchFieldRow)
            .build();
        match_rows_col = match_rows_col.add(add_btn, LayoutConstraint::Length(3));

        Element::panel(match_rows_col.build()).title("Match Fields (compound key)").build()
    };

    // Fallback button (cycle between Error/Null/Default)
    let fallback_label = format!("Fallback: {} (click to cycle)", form.fallback.label());
    let fallback_btn = Element::button(FocusId::new("resolver-fallback"), &fallback_label)
        .on_press(Msg::ResolverFormCycleFallback)
        .build();

    // Default GUID input (optional - when filled, uses Default fallback)
    let default_guid_input = Element::text_input(
        FocusId::new("resolver-default-guid"),
        &form.default_guid.value,
        &mut form.default_guid.state,
    )
    .placeholder("Optional GUID - uses this if no match found")
    .on_event(Msg::ResolverFormDefaultGuid)
    .build();
    let default_guid_panel = Element::panel(default_guid_input).title("Default GUID (optional)").build();

    // Help text
    let help_text = Element::styled_text(Line::from(vec![
        Span::styled(
            "Resolvers match source values to target records. Use multiple fields for compound key matching.",
            Style::default().fg(theme.text_tertiary),
        ),
    ]))
    .build();

    // Buttons
    let cancel_btn = Element::button(FocusId::new("resolver-cancel"), "Cancel")
        .on_press(Msg::CloseResolverModal)
        .build();

    let save_btn = if form.is_valid() {
        Element::button(FocusId::new("resolver-save"), "Save")
            .on_press(Msg::SaveResolver)
            .build()
    } else {
        Element::button(FocusId::new("resolver-save"), "Save").build()
    };

    let button_row = RowBuilder::new()
        .add(cancel_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(save_btn, LayoutConstraint::Length(12))
        .build();

    // Calculate height based on number of match field rows
    let base_height: u16 = 28; // Base modal height for single field
    let extra_rows = if rows_len > 1 { (rows_len - 1).min(3) * 4 } else { 0 };
    let modal_height = base_height + extra_rows as u16;

    let form_content = ColumnBuilder::new()
        .add(name_panel, LayoutConstraint::Length(3))
        .add(entity_panel, LayoutConstraint::Length(3))
        .add(match_field_content, LayoutConstraint::Length(if rows_len == 1 { 5 } else { 4 + rows_len.min(4) as u16 * 3 + 3 }))
        .add(fallback_btn, LayoutConstraint::Length(3))
        .add(default_guid_panel, LayoutConstraint::Length(3))
        .add(help_text, LayoutConstraint::Length(2))
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    Element::panel(Element::container(form_content).padding(1).build())
        .title(title)
        .width(65)
        .height(modal_height.min(45))
        .build()
}

pub fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = vec![];

    if state.show_delete_confirm {
        subs.push(Subscription::keyboard(KeyCode::Enter, "Confirm", Msg::ConfirmDelete));
        subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::CancelDelete));
    } else if state.show_entity_modal {
        subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::CloseEntityModal));
        subs.push(Subscription::keyboard(KeyCode::Enter, "Save", Msg::SaveEntity));
    } else if state.show_field_modal {
        subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::CloseFieldModal));
        subs.push(Subscription::keyboard(KeyCode::Enter, "Save", Msg::SaveField));
        subs.push(Subscription::ctrl_key(KeyCode::Char('t'), "Cycle transform type", Msg::FieldFormToggleType));

        // Transform-specific shortcuts
        match state.field_form.transform_type {
            TransformType::Copy => {
                subs.push(Subscription::ctrl_key(KeyCode::Char('r'), "Cycle resolver", Msg::FieldFormCycleResolver));
            }
            TransformType::Conditional => {
                subs.push(Subscription::ctrl_key(KeyCode::Char('c'), "Cycle condition", Msg::FieldFormToggleConditionType));
            }
            TransformType::ValueMap => {
                subs.push(Subscription::ctrl_key(KeyCode::Char('f'), "Cycle fallback", Msg::FieldFormToggleFallback));
                subs.push(Subscription::ctrl_key(KeyCode::Char('a'), "Add mapping", Msg::FieldFormAddMapping));
            }
            TransformType::Format => {
                subs.push(Subscription::ctrl_key(KeyCode::Char('n'), "Cycle null handling", Msg::FieldFormToggleNullHandling));
            }
            _ => {}
        }
    } else if state.show_resolver_modal {
        subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel", Msg::CloseResolverModal));
        subs.push(Subscription::keyboard(KeyCode::Enter, "Save", Msg::SaveResolver));
        subs.push(Subscription::ctrl_key(KeyCode::Char('f'), "Cycle fallback", Msg::ResolverFormCycleFallback));
        subs.push(Subscription::ctrl_key(KeyCode::Char('a'), "Add match field", Msg::ResolverAddMatchFieldRow));
        if state.resolver_form.match_field_rows.len() > 1 {
            subs.push(Subscription::ctrl_key(KeyCode::Char('d'), "Remove field", Msg::ResolverRemoveMatchFieldRow));
        }
    } else {
        // Main view subscriptions
        subs.push(Subscription::keyboard(KeyCode::Char('a'), "Add entity", Msg::AddEntity));
        subs.push(Subscription::keyboard(KeyCode::Char('r'), "Add resolver", Msg::AddResolver));
        subs.push(Subscription::keyboard(KeyCode::Esc, "Back", Msg::Back));

        // Context-sensitive actions based on selection
        if let Resource::Success(config) = &state.config {
            if let Some(selected) = state.tree_state.selected() {
                if selected.starts_with("entity_") {
                    if let Some(idx) = selected.strip_prefix("entity_").and_then(|s| s.parse::<usize>().ok()) {
                        subs.push(Subscription::keyboard(
                            KeyCode::Char('e'),
                            "Edit entity",
                            Msg::EditEntity(idx),
                        ));
                        subs.push(Subscription::keyboard(
                            KeyCode::Char('d'),
                            "Delete entity",
                            Msg::DeleteEntity(idx),
                        ));
                        subs.push(Subscription::keyboard(
                            KeyCode::Char('f'),
                            "Add field",
                            Msg::AddField(idx),
                        ));
                    }
                } else if selected.starts_with("field_") {
                    let parts: Vec<&str> = selected.strip_prefix("field_").unwrap_or("").split('_').collect();
                    if parts.len() == 2 {
                        if let (Ok(entity_idx), Ok(field_idx)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                            subs.push(Subscription::keyboard(
                                KeyCode::Char('e'),
                                "Edit field",
                                Msg::EditField(entity_idx, field_idx),
                            ));
                            subs.push(Subscription::keyboard(
                                KeyCode::Char('d'),
                                "Delete field",
                                Msg::DeleteField(entity_idx, field_idx),
                            ));
                        }
                    }
                } else if selected.starts_with("resolver_") {
                    if let Some(idx) = selected.strip_prefix("resolver_").and_then(|s| s.parse::<usize>().ok()) {
                        subs.push(Subscription::keyboard(
                            KeyCode::Char('e'),
                            "Edit resolver",
                            Msg::EditResolver(idx),
                        ));
                        subs.push(Subscription::keyboard(
                            KeyCode::Char('d'),
                            "Delete resolver",
                            Msg::DeleteResolver(idx),
                        ));
                    }
                }
            }
        }
    }

    subs
}
