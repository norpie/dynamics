//! Step 2: Entity Selection View
//!
//! Multi-select entities with filtering and junction entity panel.

use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};

use crate::tui::element::Element;
use crate::tui::widgets::ListItem;
use crate::tui::state::theme::Theme;
use crate::tui::FocusId;
use crate::tui::Resource;
use crate::{col, row, spacer, use_constraints, button_row};

use super::super::state::{State, EntityListItem, JunctionCandidate};
use super::super::msg::Msg;

/// Hardcoded presets for entity selection
pub struct EntityPreset {
    pub name: &'static str,
    pub entities: &'static [&'static str],
}

/// Available presets - first one is "None" (no preset)
pub const PRESETS: &[EntityPreset] = &[
    EntityPreset {
        name: "(No preset)",
        entities: &[],
    },
    EntityPreset {
        name: "VAF Settings",
        entities: &[
            "nrq_role",
            "nrq_country",
            "nrq_broadcaster",
            "nrq_station",
            "nrq_domain",
            "nrq_fund",
            "nrq_type",
            "nrq_support",
            "nrq_category",
            "nrq_subcategory",
            "nrq_flemishshare",
            "nrq_betalingsschijf",
            "nrq_betalingsschijflijn",
            "nrq_grootboekrekening",
            "nrq_kostenplaats",
        ],
    },
    EntityPreset {
        name: "VAF Settings (minimal)",
        entities: &[
            "nrq_fund",
            "nrq_type",
        ],
    },
    EntityPreset {
        name: "VAF Settings (junction test)",
        entities: &[
            "nrq_flemishshare",
            "nrq_category",
        ],
    },
    EntityPreset {
        name: "VAF (test)",
        entities: &[
            "cgk_category",
            "cgk_length",
        ],
    },
    EntityPreset {
        name: "VAF Settings (absolute minimal)",
        entities: &["nrq_country"],
    },
];

/// Get preset options as strings for the dropdown
pub fn preset_options() -> Vec<String> {
    PRESETS.iter().map(|p| p.name.to_string()).collect()
}

/// Entity list item for rendering (with selection checkbox)
#[derive(Clone)]
struct SelectableEntity<'a> {
    entity: &'a EntityListItem,
    is_selected: bool,
}

impl<'a> ListItem for SelectableEntity<'a> {
    type Msg = Msg;

    fn to_element(&self, is_focused: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let checkbox = if self.is_selected { "[✓]" } else { "[ ]" };

        let display_text = self.entity.display_text();
        let count_text = self.entity.record_count
            .map(|c| format!(" ({} records)", c))
            .unwrap_or_default();

        let text = format!("{} {}{}", checkbox, display_text, count_text);

        let style = if self.is_selected {
            Style::default().fg(theme.accent_success)
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

/// Junction candidate list item
#[derive(Clone)]
struct SelectableJunction<'a> {
    junction: &'a JunctionCandidate,
    is_included: bool,
}

impl<'a> ListItem for SelectableJunction<'a> {
    type Msg = Msg;

    fn to_element(&self, is_focused: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let checkbox = if self.is_included { "[✓]" } else { "[ ]" };
        let text = format!("{} {}", checkbox, self.junction.display_text());

        let style = if self.is_included {
            Style::default().fg(theme.accent_info)
        } else {
            Style::default().fg(theme.text_secondary)
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

/// Render the entity selection step
pub fn render_entity_select(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let content = match &state.entity_select.available_entities {
        Resource::NotAsked | Resource::Loading => {
            render_loading(theme)
        }
        Resource::Failure(err) => {
            render_error(err, theme)
        }
        Resource::Success(_) => {
            render_entity_lists(state, theme)
        }
    };

    let header = render_step_header(state, theme);
    let footer = render_step_footer(state, theme);

    col![
        header => Length(3),
        content => Fill(1),
        footer => Length(5),
    ]
}

/// Render loading state
fn render_loading(_theme: &Theme) -> Element<Msg> {
    Element::panel(Element::text("Loading entities from origin..."))
        .title("Entities")
        .build()
}

/// Render error state
fn render_error(err: &str, _theme: &Theme) -> Element<Msg> {
    let text = format!("Error: {}", err);
    Element::panel(Element::text(text))
        .title("Error")
        .build()
}

/// Render entity and junction lists
fn render_entity_lists(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    // Preset dropdown
    let preset_select = Element::select(
        FocusId::new("preset-select"),
        preset_options(),
        &mut state.entity_select.preset_selector,
    )
    .on_event(Msg::PresetSelectEvent)
    .build();

    let preset_panel = Element::panel(preset_select)
        .title("Preset")
        .build();

    // Filter input
    let filter_input = Element::text_input(
        FocusId::new("entity-filter"),
        &state.entity_select.filter_text,
        &state.entity_select.filter_input,
    )
    .placeholder("Type to filter entities...")
    .on_event(Msg::FilterInputEvent)
    .build();

    let filter_panel = Element::panel(filter_input)
        .title("Filter")
        .build();

    // Top row with preset and filter
    let top_row = row![
        preset_panel => Length(25),
        filter_panel => Fill(1),
    ];

    // Get filtered entities
    let filtered_entities = state.entity_select.filtered_entities();
    let entity_items: Vec<SelectableEntity> = filtered_entities.iter().map(|e| {
        SelectableEntity {
            entity: *e,
            is_selected: state.entity_select.selected_entities.contains(&e.logical_name),
        }
    }).collect();

    // Entity list
    let entity_title = format!(
        "Entities ({} selected, {} shown)",
        state.entity_select.selected_entities.len(),
        entity_items.len()
    );

    let entity_list = Element::list(
        "entity-list",
        &entity_items,
        &state.entity_select.entity_list,
        theme,
    )
    .on_select(Msg::EntityListToggle)
    .on_activate(Msg::EntityListToggle)
    .on_navigate(Msg::EntityListNavigate)
    .build();

    let entity_panel = Element::panel(entity_list)
        .title(entity_title)
        .build();

    // Build main content with optional junction panel
    if state.entity_select.show_junctions && !state.entity_select.junction_candidates.is_empty() {
        // Show junction panel
        let junction_items: Vec<SelectableJunction> = state.entity_select.junction_candidates.iter().map(|j| {
            SelectableJunction {
                junction: j,
                is_included: state.entity_select.included_junctions.contains(&j.logical_name),
            }
        }).collect();

        let junction_title = format!(
            "Junction Entities ({} found, {} included)",
            junction_items.len(),
            state.entity_select.included_junctions.len()
        );

        let junction_list = Element::list(
            "junction-list",
            &junction_items,
            &state.entity_select.junction_list,
            theme,
        )
        .on_select(Msg::JunctionListToggle)
        .on_activate(Msg::JunctionListToggle)
        .on_navigate(Msg::JunctionListNavigate)
        .build();

        // Junction select/deselect buttons
        let junction_buttons = row![
            Element::button("junction-select-all", "Select All")
                .on_press(Msg::IncludeAllJunctions)
                .build() => Fill(1),
            Element::button("junction-deselect-all", "Deselect All")
                .on_press(Msg::ExcludeAllJunctions)
                .build() => Fill(1),
        ];

        let junction_content = col![
            junction_list => Fill(1),
            junction_buttons => Length(3),
        ];

        let junction_panel = Element::panel(junction_content)
            .title(junction_title)
            .build();

        col![
            top_row => Length(3),
            row![
                entity_panel => Fill(2),
                junction_panel => Fill(1),
            ] => Fill(1),
        ]
    } else {
        // No junction panel
        let junction_hint = if !state.entity_select.junction_candidates.is_empty() {
            let text = format!("{} junction candidates available", state.entity_select.junction_candidates.len());
            Element::text(text)
        } else {
            Element::text("")
        };

        col![
            top_row => Length(3),
            entity_panel => Fill(1),
            junction_hint => Length(1),
        ]
    }
}

/// Render step header with origin/target info
fn render_step_header(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let origin = state.env_select.origin_env.as_deref().unwrap_or("?");
    let target = state.env_select.target_env.as_deref().unwrap_or("?");

    let header_text = "Step 2: Select Entities to Sync".to_string();
    let env_text = format!("From: {} → To: {}", origin, target);

    col![
        Element::styled_text(Line::from(Span::styled(
            header_text,
            Style::default().fg(theme.accent_primary).bold()
        ))).build() => Length(1),
        Element::styled_text(Line::from(Span::styled(
            env_text,
            Style::default().fg(theme.text_secondary)
        ))).build() => Length(1),
        spacer!() => Length(1),
    ]
}

/// Render step footer with navigation
fn render_step_footer(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let entities_loaded = matches!(state.entity_select.available_entities, Resource::Success(_));
    let has_selection = !state.entity_select.selected_entities.is_empty();

    // Validation/status message
    let status = if !entities_loaded {
        let text = "Loading entities...".to_string();
        Element::styled_text(Line::from(Span::styled(
            text,
            Style::default().fg(theme.text_secondary)
        ))).build()
    } else if !has_selection {
        let text = "⚠ Select at least one entity".to_string();
        Element::styled_text(Line::from(Span::styled(
            text,
            Style::default().fg(theme.accent_warning)
        ))).build()
    } else {
        let total = state.entity_select.entities_to_sync().len();
        let text = format!("✓ {} entities will be synced", total);
        Element::styled_text(Line::from(Span::styled(
            text,
            Style::default().fg(theme.accent_success)
        ))).build()
    };

    // Hide buttons during loading to prevent UB from canceling fetch
    // Show Back once loaded, show Analyze only when has selection
    if !entities_loaded {
        col![
            status => Length(1),
            spacer!() => Fill(1),
        ]
    } else if has_selection {
        let buttons = button_row![
            ("entity-back-btn", "Back", Msg::Back),
            ("entity-next-btn", "Analyze", Msg::Next),
        ];

        col![
            status => Length(1),
            spacer!() => Length(1),
            buttons => Length(3),
        ]
    } else {
        let buttons = button_row![
            ("entity-back-btn", "Back", Msg::Back),
        ];

        col![
            status => Length(1),
            spacer!() => Length(1),
            buttons => Length(3),
        ]
    }
}
