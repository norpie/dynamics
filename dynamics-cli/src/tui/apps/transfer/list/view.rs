use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::config::repository::transfer::TransferConfigSummary;
use crate::tui::element::{ColumnBuilder, FocusId};
use crate::tui::modals::ConfirmationModal;
use crate::tui::resource::Resource;
use crate::tui::widgets::{ListItem, ListState};
use crate::tui::{Element, LayeredView, LayoutConstraint, Subscription};

use super::state::{Msg, State};

impl ListItem for TransferConfigSummary {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (theme.accent_primary, Some(Style::default().bg(theme.bg_surface)))
        } else {
            (theme.text_primary, None)
        };

        let line = Line::from(vec![
            Span::styled(format!("  {:<30}", self.name), Style::default().fg(fg_color)),
            Span::styled(
                format!("{} → {}", self.source_env, self.target_env),
                Style::default().fg(theme.text_secondary),
            ),
            Span::styled(
                format!("  ({} entities)", self.entity_count),
                Style::default().fg(theme.text_tertiary),
            ),
        ]);

        let mut builder = Element::styled_text(line);
        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }
        builder.build()
    }
}

pub fn render(state: &mut State, theme: &crate::tui::Theme) -> LayeredView<Msg> {
    // Check state first to determine which view to render
    let (is_loading, is_error, error_msg, is_empty) = match &state.configs {
        Resource::NotAsked | Resource::Loading => (true, false, None, false),
        Resource::Failure(err) => (false, true, Some(err.clone()), false),
        Resource::Success(configs) => (false, false, None, configs.is_empty()),
    };

    let content = if is_loading {
        Element::text("Loading transfer configs...")
    } else if is_error {
        let err = error_msg.unwrap_or_default();
        Element::styled_text(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(theme.accent_error)),
            Span::styled(err, Style::default().fg(theme.text_primary)),
        ]))
        .build()
    } else if is_empty {
        render_empty_state(theme)
    } else {
        let configs = match &state.configs {
            Resource::Success(c) => c,
            _ => unreachable!(),
        };
        render_list(&state.list_state, configs, theme)
    };

    let main_view = Element::panel(content).title("Transfer Configs").build();

    let mut view = LayeredView::new(main_view);

    // Show delete confirmation modal using ConfirmationModal
    if state.show_delete_confirm {
        if let Some(name) = &state.selected_for_delete {
            let modal = ConfirmationModal::new(format!("Delete '{}'?", name))
                .message("This action cannot be undone.")
                .confirm_text("Delete")
                .cancel_text("Cancel")
                .on_confirm(Msg::ConfirmDelete)
                .on_cancel(Msg::CancelDelete)
                .build();
            view = view.with_app_modal(modal, crate::tui::Alignment::Center);
        }
    }

    // Show create modal
    if state.show_create_modal {
        let envs = match &state.environments {
            Resource::Success(e) => e.clone(),
            _ => vec![],
        };
        view = view.with_app_modal(
            render_create_modal(&mut state.create_form, envs, theme),
            crate::tui::Alignment::Center,
        );
    }

    // Show clone modal
    if state.show_clone_modal {
        if let Some(original_name) = &state.selected_for_clone {
            view = view.with_app_modal(
                render_clone_modal(&mut state.clone_form, original_name, theme),
                crate::tui::Alignment::Center,
            );
        }
    }

    view
}

fn render_empty_state(theme: &crate::tui::Theme) -> Element<Msg> {
    ColumnBuilder::new()
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(
            Element::styled_text(Line::from(vec![Span::styled(
                "No transfer configs found. Press F1 for help.",
                Style::default().fg(theme.text_secondary),
            )]))
            .build(),
            LayoutConstraint::Length(1),
        )
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .build()
}

fn render_list(
    list_state: &ListState,
    configs: &[TransferConfigSummary],
    theme: &crate::tui::Theme,
) -> Element<Msg> {
    Element::list(FocusId::new("config-list"), configs, list_state, theme)
        .on_select(Msg::SelectConfig)
        .on_activate(Msg::SelectConfig)
        .on_navigate(Msg::ListNavigate)
        .build()
}

fn render_create_modal(
    form: &mut super::state::CreateConfigForm,
    environments: Vec<String>,
    theme: &crate::tui::Theme,
) -> Element<Msg> {
    use crate::tui::element::RowBuilder;

    // Wrap each input in a panel with title as label
    let name_input = Element::text_input(
        FocusId::new("create-name"),
        &form.name.value,
        &mut form.name.state,
    )
    .placeholder("e.g., customer-migration")
    .on_event(Msg::CreateFormName)
    .build();
    let name_panel = Element::panel(name_input).title("Name").build();

    let source_input = Element::autocomplete(
        FocusId::new("create-source-env"),
        environments.clone(),
        form.source_env.value.clone(),
        &mut form.source_env.state,
    )
    .placeholder("Select source...")
    .on_event(Msg::CreateFormSourceEnv)
    .build();
    let source_panel = Element::panel(source_input).title("Source Environment").build();

    let target_input = Element::autocomplete(
        FocusId::new("create-target-env"),
        environments,
        form.target_env.value.clone(),
        &mut form.target_env.state,
    )
    .placeholder("Select target...")
    .on_event(Msg::CreateFormTargetEnv)
    .build();
    let target_panel = Element::panel(target_input).title("Target Environment").build();

    // Button row
    let cancel_btn = Element::button(FocusId::new("create-cancel"), "Cancel")
        .on_press(Msg::CloseCreateModal)
        .build();
    let create_btn = if form.is_valid() {
        Element::button(FocusId::new("create-save"), "Create")
            .on_press(Msg::SaveNewConfig)
            .build()
    } else {
        Element::button(FocusId::new("create-save"), "Create").build() // Disabled
    };

    let button_row = RowBuilder::new()
        .add(cancel_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(create_btn, LayoutConstraint::Length(12))
        .build();

    let form_content = ColumnBuilder::new()
        .add(name_panel, LayoutConstraint::Length(3))
        .add(source_panel, LayoutConstraint::Length(3))
        .add(target_panel, LayoutConstraint::Length(3))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    Element::panel(Element::container(form_content).padding(1).build())
        .title("New Transfer Config")
        .width(50)
        .height(20)
        .build()
}

fn render_clone_modal(
    form: &mut super::state::CloneConfigForm,
    original_name: &str,
    theme: &crate::tui::Theme,
) -> Element<Msg> {
    use crate::tui::element::{ColumnBuilder, RowBuilder};

    // Name input
    let name_input = Element::text_input(
        FocusId::new("clone-name"),
        &form.name.value,
        &mut form.name.state,
    )
    .placeholder("Enter new config name")
    .on_event(Msg::CloneFormName)
    .build();
    let name_panel = Element::panel(name_input).title("New Name").build();

    // Info text
    let info_text = Element::styled_text(Line::from(vec![
        Span::styled("Cloning: ", Style::default().fg(theme.text_secondary)),
        Span::styled(original_name.to_string(), Style::default().fg(theme.accent_primary)),
    ]))
    .build();

    // Button row
    let cancel_btn = Element::button(FocusId::new("clone-cancel"), "Cancel")
        .on_press(Msg::CloseCloneModal)
        .build();
    let clone_btn = if form.is_valid() {
        Element::button(FocusId::new("clone-save"), "Clone")
            .on_press(Msg::SaveClone)
            .build()
    } else {
        Element::button(FocusId::new("clone-save"), "Clone").build() // Disabled
    };

    let button_row = RowBuilder::new()
        .add(cancel_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(clone_btn, LayoutConstraint::Length(12))
        .build();

    let form_content = ColumnBuilder::new()
        .add(info_text, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(name_panel, LayoutConstraint::Length(3))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(button_row, LayoutConstraint::Length(3))
        .spacing(1)
        .build();

    Element::panel(Element::container(form_content).padding(1).build())
        .title("Clone Transfer Config")
        .width(50)
        .height(14)
        .build()
}

pub fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = vec![];

    if state.show_delete_confirm {
        // ConfirmationModal handles its own button clicks,
        // but we still register keyboard shortcuts for accessibility
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Confirm delete",
            Msg::ConfirmDelete,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CancelDelete,
        ));
    } else if state.show_create_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CloseCreateModal,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Save config",
            Msg::SaveNewConfig,
        ));
    } else if state.show_clone_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CloseCloneModal,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Save clone",
            Msg::SaveClone,
        ));
    } else {
        subs.push(Subscription::keyboard(
            KeyCode::Char('n'),
            "New config",
            Msg::CreateNew,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Edit config",
            Msg::EditSelected,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('d'),
            "Delete config",
            Msg::DeleteSelected,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('c'),
            "Clone config",
            Msg::CloneSelected,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('r'),
            "Refresh list",
            Msg::Refresh,
        ));
    }

    subs
}
