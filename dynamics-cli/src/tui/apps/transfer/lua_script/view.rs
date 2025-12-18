//! View rendering for LuaScriptApp

use crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::element::{ColumnBuilder, LayoutConstraint};
use crate::tui::resource::Resource;
use crate::tui::{Element, LayeredView, Subscription};

use super::state::{Msg, State};

pub fn render(state: &mut State, theme: &crate::tui::Theme) -> LayeredView<Msg> {
    let content = match &state.config {
        Resource::NotAsked | Resource::Loading => {
            Element::text("Loading config...")
        }
        Resource::Failure(err) => {
            Element::styled_text(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(theme.accent_error)),
                Span::styled(err.clone(), Style::default().fg(theme.text_primary)),
            ]))
            .build()
        }
        Resource::Success(config) => {
            render_main_view(state, config, theme)
        }
    };

    let title = format!("Lua Script - {}", state.config_name);
    let main_view = Element::panel(content).title(title).build();

    let mut view = LayeredView::new(main_view);

    // File browser modal
    if state.show_file_browser {
        view = view.with_app_modal(
            render_file_browser(state, theme),
            crate::tui::Alignment::Center,
        );
    }

    view
}

fn render_main_view(
    state: &State,
    config: &crate::transfer::TransferConfig,
    theme: &crate::tui::Theme,
) -> Element<Msg> {
    let has_script = config.lua_script.is_some();

    if !has_script {
        // No script loaded - show empty state
        render_no_script_view(theme)
    } else {
        // Script loaded - show script info and validation
        render_script_view(state, config, theme)
    }
}

fn render_no_script_view(theme: &crate::tui::Theme) -> Element<Msg> {
    let content = ColumnBuilder::new()
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(
            Element::styled_text(Line::from(vec![
                Span::styled("No Lua script loaded.", Style::default().fg(theme.text_secondary)),
            ]))
            .build(),
            LayoutConstraint::Length(1),
        )
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(
            Element::styled_text(Line::from(vec![
                Span::styled("Press ", Style::default().fg(theme.text_secondary)),
                Span::styled("[f]", Style::default().fg(theme.accent_primary)),
                Span::styled(" to select a Lua script file.", Style::default().fg(theme.text_secondary)),
            ]))
            .build(),
            LayoutConstraint::Length(1),
        )
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .build();

    Element::container(content).padding(2).build()
}

fn render_script_view(
    state: &State,
    config: &crate::transfer::TransferConfig,
    theme: &crate::tui::Theme,
) -> Element<Msg> {
    let mut rows = Vec::new();

    // Script path
    let path_text = config.lua_script_path.as_deref().unwrap_or("(embedded script)").to_string();
    rows.push((
        Element::styled_text(Line::from(vec![
            Span::styled("Script: ", Style::default().fg(theme.text_secondary)),
            Span::styled(path_text, Style::default().fg(theme.accent_primary)),
        ]))
        .build(),
        LayoutConstraint::Length(1),
    ));

    // Validation status
    let validation_line = match &state.validation {
        Resource::NotAsked => Line::from(vec![
            Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
            Span::styled("Not validated", Style::default().fg(theme.text_tertiary)),
        ]),
        Resource::Loading => Line::from(vec![
            Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
            Span::styled("Validating...", Style::default().fg(theme.accent_warning)),
        ]),
        Resource::Success(result) if result.is_valid => Line::from(vec![
            Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
            Span::styled("Valid", Style::default().fg(theme.accent_success)),
        ]),
        Resource::Success(result) => {
            let error_count = result.errors.len();
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
                Span::styled(format!("Invalid ({} errors)", error_count), Style::default().fg(theme.accent_error)),
            ])
        }
        Resource::Failure(err) => Line::from(vec![
            Span::styled("Status: ", Style::default().fg(theme.text_secondary)),
            Span::styled(format!("Error: {}", err), Style::default().fg(theme.accent_error)),
        ]),
    };
    rows.push((
        Element::styled_text(validation_line).build(),
        LayoutConstraint::Length(1),
    ));

    rows.push((Element::text(""), LayoutConstraint::Length(1)));

    // Declaration summary (if validated)
    if let Resource::Success(result) = &state.validation {
        if let Some(decl) = &result.declaration {
            rows.push((
                Element::styled_text(Line::from(vec![
                    Span::styled("Source entities:", Style::default().fg(theme.text_secondary)),
                ]))
                .build(),
                LayoutConstraint::Length(1),
            ));

            for (entity, entity_decl) in &decl.source {
                let field_count = entity_decl.fields.len();
                let text = if field_count > 0 {
                    format!("  - {} ({} fields)", entity, field_count)
                } else {
                    format!("  - {} (all fields)", entity)
                };
                rows.push((
                    Element::styled_text(Line::from(vec![
                        Span::styled(text, Style::default().fg(theme.text_primary)),
                    ]))
                    .build(),
                    LayoutConstraint::Length(1),
                ));
            }

            if decl.source.is_empty() {
                rows.push((
                    Element::styled_text(Line::from(vec![
                        Span::styled("  (none)", Style::default().fg(theme.text_tertiary)),
                    ]))
                    .build(),
                    LayoutConstraint::Length(1),
                ));
            }

            rows.push((Element::text(""), LayoutConstraint::Length(1)));

            rows.push((
                Element::styled_text(Line::from(vec![
                    Span::styled("Target entities:", Style::default().fg(theme.text_secondary)),
                ]))
                .build(),
                LayoutConstraint::Length(1),
            ));

            for (entity, entity_decl) in &decl.target {
                let field_count = entity_decl.fields.len();
                let text = if field_count > 0 {
                    format!("  - {} ({} fields)", entity, field_count)
                } else {
                    format!("  - {} (all fields)", entity)
                };
                rows.push((
                    Element::styled_text(Line::from(vec![
                        Span::styled(text, Style::default().fg(theme.text_primary)),
                    ]))
                    .build(),
                    LayoutConstraint::Length(1),
                ));
            }

            if decl.target.is_empty() {
                rows.push((
                    Element::styled_text(Line::from(vec![
                        Span::styled("  (none)", Style::default().fg(theme.text_tertiary)),
                    ]))
                    .build(),
                    LayoutConstraint::Length(1),
                ));
            }
        }

        // Show warnings
        if !result.warnings.is_empty() {
            rows.push((Element::text(""), LayoutConstraint::Length(1)));
            rows.push((
                Element::styled_text(Line::from(vec![
                    Span::styled("Warnings:", Style::default().fg(theme.accent_warning)),
                ]))
                .build(),
                LayoutConstraint::Length(1),
            ));
            for warning in &result.warnings {
                rows.push((
                    Element::styled_text(Line::from(vec![
                        Span::styled(format!("  - {}", warning), Style::default().fg(theme.accent_warning)),
                    ]))
                    .build(),
                    LayoutConstraint::Length(1),
                ));
            }
        }

        // Show errors
        if !result.errors.is_empty() {
            rows.push((Element::text(""), LayoutConstraint::Length(1)));
            rows.push((
                Element::styled_text(Line::from(vec![
                    Span::styled("Errors:", Style::default().fg(theme.accent_error)),
                ]))
                .build(),
                LayoutConstraint::Length(1),
            ));
            for error in &result.errors {
                rows.push((
                    Element::styled_text(Line::from(vec![
                        Span::styled(format!("  - {}", error), Style::default().fg(theme.accent_error)),
                    ]))
                    .build(),
                    LayoutConstraint::Length(1),
                ));
            }
        }
    }

    // Status message
    if let Some(status) = &state.status_message {
        rows.push((Element::text(""), LayoutConstraint::Length(1)));
        let color = if status.is_error { theme.accent_error } else { theme.accent_success };
        rows.push((
            Element::styled_text(Line::from(vec![
                Span::styled(status.text.clone(), Style::default().fg(color)),
            ]))
            .build(),
            LayoutConstraint::Length(1),
        ));
    }

    rows.push((Element::text(""), LayoutConstraint::Fill(1)));

    let mut builder = ColumnBuilder::new();
    for (element, constraint) in rows {
        builder = builder.add(element, constraint);
    }

    Element::container(builder.build()).padding(1).build()
}

fn render_file_browser(state: &State, theme: &crate::tui::Theme) -> Element<Msg> {
    let browser = Element::file_browser("file-browser", &state.file_browser, theme)
        .on_file_selected(Msg::FileSelected)
        .on_directory_entered(Msg::DirectoryEntered)
        .on_navigate(Msg::FileBrowserNavigate)
        .on_render(Msg::SetViewportHeight)
        .build();

    Element::panel(browser)
        .title(format!("Select Lua Script - {}", state.file_browser.current_path().display()))
        .width(70)
        .height(20)
        .build()
}

pub fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = Vec::new();

    if state.show_file_browser {
        // File browser handles its own navigation via on_navigate
        subs.push(Subscription::keyboard(KeyCode::Esc, "Close", Msg::CloseFileBrowser));
    } else {
        // Main view subscriptions
        subs.push(Subscription::keyboard(KeyCode::Esc, "Back", Msg::GoBack));
        subs.push(Subscription::keyboard(KeyCode::Char('f'), "Load file", Msg::OpenFileBrowser));
        subs.push(Subscription::keyboard(KeyCode::Char('v'), "Validate", Msg::Validate));
        subs.push(Subscription::keyboard(KeyCode::Char('p'), "Preview", Msg::StartPreview));
    }

    subs
}
