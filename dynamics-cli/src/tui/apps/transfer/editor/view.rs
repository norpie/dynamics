use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

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

    // Title with dirty indicator
    let title = if state.dirty {
        format!("{} *", state.config_name)
    } else {
        state.config_name.clone()
    };

    let main_view = Element::panel(content).title(&title).build();
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
        view = view.with_app_modal(
            render_field_modal(&mut state.field_form, state.editing_field.map(|(_, f)| f != usize::MAX).unwrap_or(false), theme),
            Alignment::Center,
        );
    }

    // Delete confirmation
    if state.show_delete_confirm {
        let message = match &state.delete_target {
            Some(DeleteTarget::Entity(_)) => "Delete this entity mapping and all its field mappings?",
            Some(DeleteTarget::Field(_, _)) => "Delete this field mapping?",
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

    let save_btn = if state.dirty {
        Element::button(FocusId::new("save-btn"), "Save")
            .on_press(Msg::Save)
            .build()
    } else {
        Element::button(FocusId::new("save-btn"), "Save").build()
    };

    let preview_btn = Element::button(FocusId::new("preview-btn"), "Preview")
        .on_press(Msg::Preview)
        .build();

    let button_row = RowBuilder::new()
        .add(back_btn, LayoutConstraint::Length(10))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(save_btn, LayoutConstraint::Length(10))
        .add(Element::text(""), LayoutConstraint::Length(1))
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
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    Element::panel(Element::container(form_content).padding(1).build())
        .title(title)
        .width(50)
        .height(20)
        .build()
}

fn render_field_modal(
    form: &mut super::state::FieldMappingForm,
    is_edit: bool,
    theme: &Theme,
) -> Element<Msg> {
    let title = if is_edit { "Edit Field Mapping" } else { "Add Field Mapping" };

    // Target field input
    let target_input = Element::text_input(
        FocusId::new("field-target"),
        &form.target_field.value,
        &mut form.target_field.state,
    )
    .placeholder("e.g., name")
    .on_event(Msg::FieldFormTarget)
    .build();
    let target_panel = Element::panel(target_input).title("Target Field").build();

    // Transform type toggle
    let type_label = match form.transform_type {
        TransformType::Copy => "Copy (press Tab to toggle)",
        TransformType::Constant => "Constant (press Tab to toggle)",
    };
    let type_indicator = Element::styled_text(Line::from(vec![
        Span::styled("Transform: ", Style::default().fg(theme.text_tertiary)),
        Span::styled(type_label, Style::default().fg(theme.accent_primary)),
    ]))
    .build();

    // Source path or constant value depending on type
    let value_panel = match form.transform_type {
        TransformType::Copy => {
            let input = Element::text_input(
                FocusId::new("field-source"),
                &form.source_path.value,
                &mut form.source_path.state,
            )
            .placeholder("e.g., name or accountid.name")
            .on_event(Msg::FieldFormSourcePath)
            .build();
            Element::panel(input).title("Source Path").build()
        }
        TransformType::Constant => {
            let input = Element::text_input(
                FocusId::new("field-constant"),
                &form.constant_value.value,
                &mut form.constant_value.state,
            )
            .placeholder("e.g., true, 42, or string")
            .on_event(Msg::FieldFormConstant)
            .build();
            Element::panel(input).title("Constant Value").build()
        }
    };

    // Buttons
    let cancel_btn = Element::button(FocusId::new("field-cancel"), "Cancel")
        .on_press(Msg::CloseFieldModal)
        .build();

    let save_btn = if form.is_valid() {
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

    let form_content = ColumnBuilder::new()
        .add(target_panel, LayoutConstraint::Length(3))
        .add(type_indicator, LayoutConstraint::Length(1))
        .add(value_panel, LayoutConstraint::Length(3))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    Element::panel(Element::container(form_content).padding(1).build())
        .title(title)
        .width(55)
        .height(18)
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
        subs.push(Subscription::keyboard(KeyCode::Tab, "Toggle type", Msg::FieldFormToggleType));
    } else {
        // Main view subscriptions
        subs.push(Subscription::keyboard(KeyCode::Char('a'), "Add entity", Msg::AddEntity));
        subs.push(Subscription::keyboard(KeyCode::Char('s'), "Save", Msg::Save));
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
                }
            }
        }
    }

    subs
}
