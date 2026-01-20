use crate::tui::{Element, Theme, widgets::TreeItem};
use serde_json::Value;

/// Badge type for copy semantics
#[derive(Clone, Debug, PartialEq)]
pub enum CopyBadge {
    Copy,      // Green - entity will be copied with new GUID
    Reference, // Blue - entity will be referenced (keep same GUID)
    Junction,  // Yellow - junction record (will be copied)
    Remap,     // Orange - contains IDs that must be remapped
}

impl CopyBadge {
    pub fn label(&self) -> &'static str {
        match self {
            CopyBadge::Copy => "COPY",
            CopyBadge::Reference => "REF",
            CopyBadge::Junction => "JCT",
            CopyBadge::Remap => "REMAP",
        }
    }

    pub fn color(&self, theme: &Theme) -> ratatui::style::Color {
        match self {
            CopyBadge::Copy => theme.accent_success,
            CopyBadge::Reference => theme.accent_info,
            CopyBadge::Junction => theme.accent_warning,
            CopyBadge::Remap => theme.accent_error,
        }
    }
}

/// Tree items for questionnaire snapshot visualization
#[derive(Clone)]
pub enum SnapshotTreeItem {
    /// Root questionnaire node
    QuestionnaireRoot {
        name: String,
        id: String,
        badge: Option<CopyBadge>,
        children: Vec<SnapshotTreeItem>,
    },
    /// Category grouping (e.g., "Pages (3)", "Conditions (5)")
    Category {
        id: String,   // Unique ID for this category (e.g., "fields:questionnaire-abc")
        name: String, // Display name (e.g., "Fields", "Pages")
        count: usize,
        children: Vec<SnapshotTreeItem>,
    },
    /// Regular entity (page, group, question, condition, etc.)
    Entity {
        name: String,
        id: String,
        badge: Option<CopyBadge>,
        children: Vec<SnapshotTreeItem>,
    },
    /// Junction record (page line, group line, template line)
    JunctionRecord {
        name: String,
        id: String,
        description: Option<String>, // e.g., "Order: 1, Create Questions: true"
        children: Vec<SnapshotTreeItem>,
    },
    /// Referenced entity (template, tag, classification - not copied)
    ReferencedEntity {
        name: String,
        id: String,
        entity_type: String, // e.g., "nrq_questiontemplate", "nrq_category"
    },
    /// Field attribute
    Attribute {
        parent_id: String, // Parent entity ID to ensure uniqueness
        label: String,
        value: String,
    },
    /// Parsed condition logic (with REMAP warning)
    ConditionLogicInfo {
        trigger_question_id: String,
        condition_operator: String,
        value: String,
        affected_count: usize,
        details: Vec<String>, // Formatted details for each affected question
    },
}

impl TreeItem for SnapshotTreeItem {
    type Msg = super::Msg;

    fn id(&self) -> String {
        match self {
            Self::QuestionnaireRoot { id, .. } => format!("root:{}", id),
            Self::Category { id, .. } => id.clone(),
            Self::Entity { id, .. } => format!("entity:{}", id),
            Self::JunctionRecord { id, .. } => format!("junction:{}", id),
            Self::ReferencedEntity {
                id, entity_type, ..
            } => format!("ref:{}:{}", entity_type, id),
            Self::Attribute {
                parent_id,
                label,
                value,
            } => {
                // Generate stable ID based on parent, label, and value
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                parent_id.hash(&mut hasher);
                label.hash(&mut hasher);
                value.hash(&mut hasher);
                format!("attr:{}", hasher.finish())
            }
            Self::ConditionLogicInfo {
                trigger_question_id,
                ..
            } => {
                format!("condlogic:{}", trigger_question_id)
            }
        }
    }

    fn has_children(&self) -> bool {
        match self {
            Self::QuestionnaireRoot { children, .. } => !children.is_empty(),
            Self::Category { children, .. } => !children.is_empty(),
            Self::Entity { children, .. } => !children.is_empty(),
            Self::JunctionRecord { children, .. } => !children.is_empty(),
            Self::ReferencedEntity { .. } => false,
            Self::Attribute { .. } => false,
            Self::ConditionLogicInfo { details, .. } => !details.is_empty(),
        }
    }

    fn children(&self) -> Vec<Self> {
        match self {
            Self::QuestionnaireRoot { children, .. } => children.clone(),
            Self::Category { children, .. } => children.clone(),
            Self::Entity { children, .. } => children.clone(),
            Self::JunctionRecord { children, .. } => children.clone(),
            Self::ReferencedEntity { .. } => vec![],
            Self::Attribute { .. } => vec![],
            Self::ConditionLogicInfo {
                trigger_question_id,
                details,
                ..
            } => {
                // Convert details into Attribute nodes
                details
                    .iter()
                    .enumerate()
                    .map(|(i, detail)| Self::Attribute {
                        parent_id: format!("condlogic:{}", trigger_question_id),
                        label: format!("Target {}", i + 1),
                        value: detail.clone(),
                    })
                    .collect()
            }
        }
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        _is_multi_selected: bool,
        is_expanded: bool,
    ) -> Element<Self::Msg> {
        use ratatui::{
            prelude::Stylize,
            style::Style,
            text::{Line, Span},
        };

        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);

        match self {
            Self::QuestionnaireRoot {
                name,
                badge,
                children,
                ..
            } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Expand/collapse indicator
                if !children.is_empty() {
                    let indicator = if is_expanded { "▼ " } else { "▶ " };
                    spans.push(Span::styled(
                        indicator,
                        Style::default().fg(theme.border_primary),
                    ));
                }

                // Questionnaire icon
                spans.push(Span::styled("📋 ", Style::default()));

                // Questionnaire name
                spans.push(Span::styled(
                    name.clone(),
                    Style::default().fg(theme.accent_info).bold(),
                ));

                // Badge if present
                if let Some(badge) = badge {
                    spans.push(Span::styled(" ", Style::default()));
                    spans.push(Span::styled(
                        format!("[{}]", badge.label()),
                        Style::default().fg(badge.color(theme)),
                    ));
                }

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::Category {
                name,
                count,
                children,
                ..
            } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Expand/collapse indicator for categories with children
                if !children.is_empty() {
                    let indicator = if is_expanded { "▼ " } else { "▶ " };
                    spans.push(Span::styled(
                        indicator,
                        Style::default().fg(theme.border_primary),
                    ));
                }

                // Category name with count
                spans.push(Span::styled(
                    format!("{} ({})", name, count),
                    Style::default().fg(theme.text_primary).bold(),
                ));

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::Entity {
                name,
                badge,
                children,
                ..
            } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Expand/collapse indicator if entity has children
                if !children.is_empty() {
                    let indicator = if is_expanded { "▼ " } else { "▶ " };
                    spans.push(Span::styled(
                        indicator,
                        Style::default().fg(theme.border_primary),
                    ));
                }

                // Entity name
                spans.push(Span::styled(
                    name.clone(),
                    Style::default().fg(theme.text_secondary),
                ));

                // Badge if present
                if let Some(badge) = badge {
                    spans.push(Span::styled(" ", Style::default()));
                    spans.push(Span::styled(
                        format!("[{}]", badge.label()),
                        Style::default().fg(badge.color(theme)),
                    ));
                }

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::JunctionRecord {
                name,
                description,
                children,
                ..
            } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Expand/collapse indicator if has children
                if !children.is_empty() {
                    let indicator = if is_expanded { "▼ " } else { "▶ " };
                    spans.push(Span::styled(
                        indicator,
                        Style::default().fg(theme.border_primary),
                    ));
                }

                // Junction indicator
                spans.push(Span::styled(
                    "⚡ ",
                    Style::default().fg(theme.accent_warning),
                ));

                // Junction name
                spans.push(Span::styled(
                    name.clone(),
                    Style::default().fg(theme.text_secondary),
                ));

                // Badge
                spans.push(Span::styled(" ", Style::default()));
                spans.push(Span::styled(
                    "[JCT]",
                    Style::default().fg(theme.accent_warning),
                ));

                // Description if present
                if let Some(desc) = description {
                    spans.push(Span::styled(
                        format!(" - {}", desc),
                        Style::default().fg(theme.text_tertiary),
                    ));
                }

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::ReferencedEntity {
                name, entity_type, ..
            } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Reference indicator
                spans.push(Span::styled("→ ", Style::default().fg(theme.accent_info)));

                // Entity name
                spans.push(Span::styled(
                    name.clone(),
                    Style::default().fg(theme.text_secondary),
                ));

                // Badge
                spans.push(Span::styled(" ", Style::default()));
                spans.push(Span::styled(
                    "[REF]",
                    Style::default().fg(theme.accent_info),
                ));

                // Entity type in parentheses
                spans.push(Span::styled(
                    format!(" ({})", entity_type),
                    Style::default().fg(theme.text_tertiary),
                ));

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::Attribute { label, value, .. } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Label in dim color
                spans.push(Span::styled(
                    format!("{}: ", label),
                    Style::default().fg(theme.text_tertiary),
                ));

                // Value
                spans.push(Span::styled(
                    value.clone(),
                    Style::default().fg(theme.text_secondary),
                ));

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
            Self::ConditionLogicInfo {
                trigger_question_id,
                condition_operator,
                value,
                affected_count,
                details,
            } => {
                let mut spans = Vec::new();

                // Indent
                if depth > 0 {
                    spans.push(Span::styled(indent, Style::default()));
                }

                // Expand/collapse indicator if has details
                if !details.is_empty() {
                    let indicator = if is_expanded { "▼ " } else { "▶ " };
                    spans.push(Span::styled(
                        indicator,
                        Style::default().fg(theme.border_primary),
                    ));
                }

                // Warning icon
                spans.push(Span::styled("⚠️  ", Style::default()));

                // Summary
                spans.push(Span::styled(
                    format!(
                        "IF Question {} {} \"{}\" THEN affect {} question(s)",
                        &trigger_question_id[..8.min(trigger_question_id.len())],
                        condition_operator,
                        value,
                        affected_count
                    ),
                    Style::default().fg(theme.accent_error),
                ));

                // REMAP badge
                spans.push(Span::styled(" ", Style::default()));
                spans.push(Span::styled(
                    format!("[{} IDs]", affected_count + 1),
                    Style::default().fg(theme.accent_error).bold(),
                ));

                let mut builder = Element::styled_text(Line::from(spans));

                if is_selected {
                    builder = builder.background(Style::default().bg(theme.bg_surface));
                }

                builder.build()
            }
        }
    }
}
