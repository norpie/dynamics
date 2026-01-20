//! TreeItem implementations for entity comparison

use super::{MatchInfo, MatchType};
use crate::api::{FieldMetadata, FormMetadata, RelationshipMetadata, ViewMetadata};
use crate::tui::{Element, Theme, widgets::TreeItem};
use ratatui::{
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
};

/// Unified tree item that can represent any metadata type
#[derive(Clone)]
pub enum ComparisonTreeItem {
    Container(ContainerNode),
    Field(FieldNode),
    Relationship(RelationshipNode),
    View(ViewNode),
    Form(FormNode),
    Entity(EntityNode),
}

impl TreeItem for ComparisonTreeItem {
    type Msg = super::Msg;

    fn id(&self) -> String {
        match self {
            Self::Container(node) => node.id.clone(),
            Self::Field(node) => node.id(),
            Self::Relationship(node) => node.id(),
            Self::View(node) => node.id(),
            Self::Form(node) => node.id(),
            Self::Entity(node) => node.id(),
        }
    }

    fn has_children(&self) -> bool {
        match self {
            Self::Container(node) => !node.children.is_empty(),
            Self::Field(node) => node.has_children(),
            Self::Relationship(node) => node.has_children(),
            Self::View(node) => node.has_children(),
            Self::Form(node) => node.has_children(),
            Self::Entity(node) => node.has_children(),
        }
    }

    fn children(&self) -> Vec<Self> {
        match self {
            Self::Container(node) => node.children.clone(),
            Self::Field(node) => node.children().into_iter().map(Self::Field).collect(),
            Self::Relationship(node) => node
                .children()
                .into_iter()
                .map(Self::Relationship)
                .collect(),
            Self::View(node) => node.children().into_iter().map(Self::View).collect(),
            Self::Form(node) => node.children().into_iter().map(Self::Form).collect(),
            Self::Entity(node) => node.children().into_iter().map(Self::Entity).collect(),
        }
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        is_expanded: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        match self {
            Self::Container(node) => {
                let indent = "  ".repeat(depth);
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Multi-select checkmark indicator
                if is_multi_selected {
                    spans.push(Span::styled(
                        "✓ ",
                        Style::default().fg(theme.accent_primary),
                    ));
                }

                // Use stored container_match_type for color (keep color even when selected)
                let color = match node.container_match_type {
                    ContainerMatchType::FullMatch => theme.accent_success,
                    ContainerMatchType::Mixed => theme.accent_warning,
                    ContainerMatchType::NoMatch => theme.accent_error,
                };

                // Container label
                spans.push(Span::styled(
                    node.label.clone(),
                    Style::default().fg(color).bold(),
                ));

                // Show match info if container has a mapping
                if let Some(match_info) = &node.match_info {
                    spans.push(Span::styled(
                        " → ",
                        Style::default().fg(theme.border_primary),
                    ));

                    // Extract just the container name from target paths and format as comma-separated
                    let target_displays: Vec<String> = match_info
                        .target_fields
                        .iter()
                        .map(|tf| tf.split('/').last().unwrap_or(tf).to_string())
                        .collect();

                    let target_display = target_displays.join(", ");

                    spans.push(Span::styled(
                        target_display,
                        Style::default().fg(theme.accent_secondary),
                    ));

                    // Show match type of primary target
                    if let Some(primary) = match_info.primary_target() {
                        if let Some(match_type) = match_info.match_types.get(primary) {
                            spans.push(Span::styled(
                                format!(" {}", match_type.label()),
                                Style::default().fg(theme.border_primary),
                            ));
                        }
                    }
                }

                let mut builder = Element::styled_text(Line::from(spans));

                // Background: multi-selected items get elevated color, primary selection gets surface color
                if is_multi_selected {
                    builder = builder.background(Style::default().bg(theme.bg_elevated));
                } else if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::Field(node) => {
                node.to_element(depth, is_selected, is_multi_selected, is_expanded)
            }
            Self::Relationship(node) => {
                node.to_element(depth, is_selected, is_multi_selected, is_expanded)
            }
            Self::View(node) => node.to_element(depth, is_selected, is_multi_selected, is_expanded),
            Self::Form(node) => node.to_element(depth, is_selected, is_multi_selected, is_expanded),
            Self::Entity(node) => {
                node.to_element(depth, is_selected, is_multi_selected, is_expanded)
            }
        }
    }
}

/// Generic container node (for FormType, Form, Tab, Section, ViewType, View, etc.)
#[derive(Clone)]
pub struct ContainerNode {
    pub id: String,
    pub label: String,
    pub children: Vec<ComparisonTreeItem>,
    pub container_match_type: ContainerMatchType, // Unmapped, FullMatch, or Mixed
    pub match_info: Option<MatchInfo>, // Match info if this container is manually/automatically mapped
}

/// Container match type (aggregated from children)
#[derive(Clone, Debug, PartialEq)]
pub enum ContainerMatchType {
    NoMatch,   // Container not matched
    FullMatch, // Container matched AND all children matched
    Mixed,     // Container matched BUT not all children matched
}

/// Truncate a value string to a maximum length for display
fn truncate_value(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}...", &value[..max_len.saturating_sub(3)])
    }
}

/// Field node in the tree
#[derive(Clone)]
pub struct FieldNode {
    pub metadata: FieldMetadata,
    pub match_info: Option<MatchInfo>,
    pub example_value: Option<String>,
    pub display_name: String, // Computed name to display (technical or friendly)
    pub is_ignored: bool,
}

impl TreeItem for FieldNode {
    type Msg = super::Msg;

    fn id(&self) -> String {
        self.metadata.logical_name.clone()
    }

    fn has_children(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<Self> {
        vec![]
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);
        let mut spans = Vec::new();

        // Indent
        if depth > 0 {
            spans.push(Span::styled(indent, Style::default()));
        }

        // Multi-select checkmark indicator
        if is_multi_selected {
            spans.push(Span::styled(
                "✓ ",
                Style::default().fg(theme.accent_primary),
            ));
        }

        // Field name - colored by match state (keep color even when selected)
        // If ignored, override with gray
        let field_name_color = if self.is_ignored {
            theme.text_tertiary // Gray for ignored items
        } else if let Some(match_info) = &self.match_info {
            // Get match type of primary target
            let primary_match_type = match_info
                .primary_target()
                .and_then(|primary| match_info.match_types.get(primary))
                .cloned();

            match primary_match_type {
                Some(MatchType::Exact) => theme.accent_success, // Exact name + type match
                Some(MatchType::Prefix) => theme.accent_success, // Prefix name + type match
                Some(MatchType::Manual) => theme.accent_success, // User override
                Some(MatchType::Import) => theme.accent_success, // Imported from C# file
                Some(MatchType::ExampleValue) => theme.palette_4, // Example value match
                Some(MatchType::TypeMismatch(_)) => theme.accent_warning, // Name match but type differs
                None => theme.accent_error,                               // No match
            }
        } else {
            theme.accent_error // No match
        };

        let field_name_style = Style::default().fg(field_name_color);

        // Use the pre-computed display name (which can be either technical/logical or user-friendly)
        spans.push(Span::styled(self.display_name.clone(), field_name_style));

        // Required indicator (red asterisk)
        if self.metadata.is_required {
            spans.push(Span::styled(" *", Style::default().fg(theme.accent_error)));
        }

        // Mapping arrow and target fields (if mapped)
        if let Some(match_info) = &self.match_info {
            spans.push(Span::styled(
                " → ",
                Style::default().fg(theme.border_primary),
            ));

            // Extract just the field name from target paths and format as comma-separated
            let target_displays: Vec<String> = match_info
                .target_fields
                .iter()
                .map(|tf| tf.split('/').last().unwrap_or(tf).to_string())
                .collect();

            let target_display = target_displays.join(", ");

            spans.push(Span::styled(
                target_display,
                Style::default().fg(theme.accent_secondary),
            ));
        }

        // Field type in angle brackets
        spans.push(Span::styled(
            format!(" <{:?}>", self.metadata.field_type),
            Style::default().fg(theme.border_primary),
        ));

        // Mapping source badge (if mapped)
        if let Some(match_info) = &self.match_info {
            // Show match type of primary target
            if let Some(primary) = match_info.primary_target() {
                if let Some(match_type) = match_info.match_types.get(primary) {
                    spans.push(Span::styled(
                        format!(" {}", match_type.label()),
                        Style::default().fg(theme.border_primary),
                    ));
                }
            }
        }

        // Example value (if present)
        if let Some(example) = &self.example_value {
            spans.push(Span::styled(
                " | ",
                Style::default().fg(theme.border_primary),
            ));
            spans.push(Span::styled(
                truncate_value(example, 60),
                Style::default().fg(theme.palette_4),
            ));
        }

        let mut builder = Element::styled_text(Line::from(spans));

        // Background: multi-selected items get elevated color, primary selection gets surface color
        if is_multi_selected {
            builder = builder.background(Style::default().bg(theme.bg_elevated));
        } else if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Relationship node in the tree
#[derive(Clone)]
pub struct RelationshipNode {
    pub metadata: RelationshipMetadata,
    pub match_info: Option<MatchInfo>,
    pub is_ignored: bool,
}

impl TreeItem for RelationshipNode {
    type Msg = super::Msg;

    fn id(&self) -> String {
        format!("rel_{}", self.metadata.name)
    }

    fn has_children(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<Self> {
        vec![]
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);
        let mut spans = Vec::new();

        // Indent
        if depth > 0 {
            spans.push(Span::styled(indent, Style::default()));
        }

        // Multi-select checkmark indicator
        if is_multi_selected {
            spans.push(Span::styled(
                "✓ ",
                Style::default().fg(theme.accent_primary),
            ));
        }

        // Relationship name - colored by match state
        // If ignored, override with gray
        let rel_name_color = if self.is_ignored {
            theme.text_tertiary // Gray for ignored items
        } else if let Some(match_info) = &self.match_info {
            // Get match type of primary target
            let primary_match_type = match_info
                .primary_target()
                .and_then(|primary| match_info.match_types.get(primary))
                .cloned();

            match primary_match_type {
                Some(MatchType::Exact) => theme.accent_success, // Exact name + type match
                Some(MatchType::Prefix) => theme.accent_success, // Prefix name + type match
                Some(MatchType::Manual) => theme.accent_success, // User override
                Some(MatchType::Import) => theme.accent_success, // Imported from C# file
                Some(MatchType::ExampleValue) => theme.palette_4, // Example value match
                Some(MatchType::TypeMismatch(_)) => theme.accent_warning, // Name match but type differs
                None => theme.accent_error,                               // No match
            }
        } else {
            theme.accent_error // No match
        };

        spans.push(Span::styled(
            self.metadata.name.clone(),
            Style::default().fg(rel_name_color),
        ));

        // Mapping arrow and target relationships (if mapped)
        if let Some(match_info) = &self.match_info {
            spans.push(Span::styled(
                " → ",
                Style::default().fg(theme.border_primary),
            ));

            // Show comma-separated targets
            let target_display = match_info.target_fields.join(", ");

            spans.push(Span::styled(
                target_display,
                Style::default().fg(theme.accent_secondary),
            ));
        }

        // Related entity and relationship type in angle brackets
        // Format: <entity [ManyToOne]> or <unknown [OneToMany]>
        let rel_type_label = match self.metadata.relationship_type {
            crate::api::metadata::RelationshipType::ManyToOne => "N:1",
            crate::api::metadata::RelationshipType::OneToMany => "1:N",
            crate::api::metadata::RelationshipType::ManyToMany => "N:N",
        };

        let entity_display = if self.metadata.related_entity == "unknown"
            || self.metadata.related_entity.is_empty()
        {
            format!(" <{}>", rel_type_label)
        } else {
            format!(" <{} {}>", self.metadata.related_entity, rel_type_label)
        };

        spans.push(Span::styled(
            entity_display,
            Style::default().fg(theme.border_primary),
        ));

        // Mapping source badge (if mapped)
        if let Some(match_info) = &self.match_info {
            // Show match type of primary target
            if let Some(primary) = match_info.primary_target() {
                if let Some(match_type) = match_info.match_types.get(primary) {
                    spans.push(Span::styled(
                        format!(" {}", match_type.label()),
                        Style::default().fg(theme.border_primary),
                    ));
                }
            }
        }

        let mut builder = Element::styled_text(Line::from(spans));

        // Background: multi-selected items get secondary color, primary selection gets surface color
        if is_multi_selected {
            builder = builder.background(Style::default().bg(theme.bg_elevated));
        } else if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// View node in the tree
#[derive(Clone)]
pub struct ViewNode {
    pub metadata: ViewMetadata,
    pub is_ignored: bool,
}

impl TreeItem for ViewNode {
    type Msg = super::Msg;

    fn id(&self) -> String {
        format!("view_{}", self.metadata.id)
    }

    fn has_children(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<Self> {
        vec![]
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        // TODO: Implement view rendering

        let indent = "  ".repeat(depth);
        let mut text = String::new();

        if depth > 0 {
            text.push_str(&indent);
        }

        if is_multi_selected {
            text.push_str("✓ ");
        }

        text.push_str(&self.metadata.name);

        let mut builder = Element::styled_text(Line::from(Span::styled(
            text,
            if is_selected || is_multi_selected {
                Style::default().fg(theme.accent_primary)
            } else {
                Style::default().fg(theme.text_primary)
            },
        )));

        if is_multi_selected {
            builder = builder.background(Style::default().bg(theme.bg_elevated));
        } else if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Form node in the tree
#[derive(Clone)]
pub struct FormNode {
    pub metadata: FormMetadata,
    pub is_ignored: bool,
}

impl TreeItem for FormNode {
    type Msg = super::Msg;

    fn id(&self) -> String {
        format!("form_{}", self.metadata.id)
    }

    fn has_children(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<Self> {
        vec![]
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        // TODO: Implement form rendering

        let indent = "  ".repeat(depth);
        let mut text = String::new();

        if depth > 0 {
            text.push_str(&indent);
        }

        if is_multi_selected {
            text.push_str("✓ ");
        }

        text.push_str(&self.metadata.name);

        let mut builder = Element::styled_text(Line::from(Span::styled(
            text,
            if is_selected || is_multi_selected {
                Style::default().fg(theme.accent_primary)
            } else {
                Style::default().fg(theme.text_primary)
            },
        )));

        if is_multi_selected {
            builder = builder.background(Style::default().bg(theme.bg_elevated));
        } else if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Entity node in the tree (for entity type mapping)
#[derive(Clone)]
pub struct EntityNode {
    pub name: String,
    pub match_info: Option<MatchInfo>,
    pub usage_count: usize,
    pub is_ignored: bool,
}

impl TreeItem for EntityNode {
    type Msg = super::Msg;

    fn id(&self) -> String {
        format!("entity_{}", self.name)
    }

    fn has_children(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<Self> {
        vec![]
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);
        let mut spans = Vec::new();

        // Indent
        if depth > 0 {
            spans.push(Span::styled(indent, Style::default()));
        }

        // Multi-select checkmark indicator
        if is_multi_selected {
            spans.push(Span::styled(
                "✓ ",
                Style::default().fg(theme.accent_primary),
            ));
        }

        // Entity name - colored by match state (keep color even when selected)
        // If ignored, override with gray
        let entity_name_color = if self.is_ignored {
            theme.text_tertiary // Gray for ignored items
        } else if let Some(match_info) = &self.match_info {
            // Get match type of primary target
            let primary_match_type = match_info
                .primary_target()
                .and_then(|primary| match_info.match_types.get(primary))
                .cloned();

            match primary_match_type {
                Some(MatchType::Exact) => theme.accent_success, // Exact name match
                Some(MatchType::Prefix) => theme.accent_success, // Prefix name match
                Some(MatchType::Manual) => theme.accent_success, // User override
                Some(MatchType::Import) => theme.accent_success, // Imported from C# file
                Some(MatchType::ExampleValue) => theme.palette_4, // Example value match
                Some(MatchType::TypeMismatch(_)) => theme.accent_warning, // Should not happen for entities
                None => theme.accent_error,                               // No match
            }
        } else {
            theme.accent_error // No match
        };

        let entity_name_style = Style::default().fg(entity_name_color);

        spans.push(Span::styled(self.name.clone(), entity_name_style));

        // Usage count
        spans.push(Span::styled(
            format!(" ({} uses)", self.usage_count),
            Style::default().fg(theme.border_primary),
        ));

        // Mapping arrow and target entities (if mapped)
        if let Some(match_info) = &self.match_info {
            spans.push(Span::styled(
                " → ",
                Style::default().fg(theme.border_primary),
            ));

            // Show comma-separated targets
            let target_display = match_info.target_fields.join(", ");

            spans.push(Span::styled(
                target_display,
                Style::default().fg(theme.accent_secondary),
            ));
        }

        // Mapping source badge (if mapped)
        if let Some(match_info) = &self.match_info {
            // Show match type of primary target
            if let Some(primary) = match_info.primary_target() {
                if let Some(match_type) = match_info.match_types.get(primary) {
                    spans.push(Span::styled(
                        format!(" {}", match_type.label()),
                        Style::default().fg(theme.border_primary),
                    ));
                }
            }
        }

        let mut builder = Element::styled_text(Line::from(spans));

        // Background: multi-selected items get secondary color, primary selection gets surface color
        if is_multi_selected {
            builder = builder.background(Style::default().bg(theme.bg_elevated));
        } else if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}
