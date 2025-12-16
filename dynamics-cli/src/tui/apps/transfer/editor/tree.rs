use crate::transfer::{EntityMapping, FieldMapping, Resolver};
use crate::tui::{Element, widgets::TreeItem};
use ratatui::{style::Style, text::{Line, Span}};

use super::state::Msg;

/// Tree item for the mapping editor
#[derive(Clone)]
pub enum MappingTreeItem {
    Resolver(ResolverNode),
    Entity(EntityNode),
    Field(FieldNode),
}

impl TreeItem for MappingTreeItem {
    type Msg = Msg;

    fn id(&self) -> String {
        match self {
            Self::Resolver(node) => node.id(),
            Self::Entity(node) => node.id(),
            Self::Field(node) => node.id(),
        }
    }

    fn has_children(&self) -> bool {
        match self {
            Self::Resolver(_) => false,
            Self::Entity(node) => !node.field_mappings.is_empty(),
            Self::Field(_) => false,
        }
    }

    fn children(&self) -> Vec<Self> {
        match self {
            Self::Resolver(_) => vec![],
            Self::Entity(node) => node
                .field_mappings
                .iter()
                .enumerate()
                .map(|(idx, fm)| {
                    Self::Field(FieldNode {
                        entity_idx: node.idx,
                        field_idx: idx,
                        mapping: fm.clone(),
                    })
                })
                .collect(),
            Self::Field(_) => vec![],
        }
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        is_expanded: bool,
    ) -> Element<Self::Msg> {
        match self {
            Self::Resolver(node) => node.to_element(depth, is_selected, is_multi_selected, is_expanded),
            Self::Entity(node) => node.to_element(depth, is_selected, is_multi_selected, is_expanded),
            Self::Field(node) => node.to_element(depth, is_selected, is_multi_selected, is_expanded),
        }
    }
}

/// Node representing a resolver in the tree
#[derive(Clone)]
pub struct ResolverNode {
    pub idx: usize,
    pub name: String,
    pub source_entity: String,
    pub match_field: String,
}

impl ResolverNode {
    pub fn from_resolver(idx: usize, resolver: &Resolver) -> Self {
        Self {
            idx,
            name: resolver.name.clone(),
            source_entity: resolver.source_entity.clone(),
            match_field: resolver.match_field.clone(),
        }
    }

    fn id(&self) -> String {
        format!("resolver_{}", self.idx)
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        _is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);

        let mut spans = Vec::new();

        if depth > 0 {
            spans.push(Span::styled(indent, Style::default()));
        }

        // No expand indicator (resolvers have no children)
        spans.push(Span::styled("  ", Style::default()));

        // [Resolver] badge
        spans.push(Span::styled(
            "[Resolver] ",
            Style::default().fg(theme.accent_warning),
        ));

        // Resolver name
        spans.push(Span::styled(
            self.name.clone(),
            Style::default().fg(theme.text_primary),
        ));

        // Arrow
        spans.push(Span::styled(" → ", Style::default().fg(theme.border_primary)));

        // Source entity
        spans.push(Span::styled(
            self.source_entity.clone(),
            Style::default().fg(theme.accent_secondary),
        ));

        // Match field in parentheses
        spans.push(Span::styled(
            format!(" ({})", self.match_field),
            Style::default().fg(theme.text_tertiary),
        ));

        let mut builder = Element::styled_text(Line::from(spans));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

#[derive(Clone)]
pub struct EntityNode {
    pub idx: usize,
    pub source_entity: String,
    pub target_entity: String,
    pub priority: u32,
    pub field_mappings: Vec<FieldMapping>,
}

impl EntityNode {
    pub fn from_mapping(idx: usize, mapping: &EntityMapping) -> Self {
        Self {
            idx,
            source_entity: mapping.source_entity.clone(),
            target_entity: mapping.target_entity.clone(),
            priority: mapping.priority,
            field_mappings: mapping.field_mappings.clone(),
        }
    }

    fn id(&self) -> String {
        format!("entity_{}", self.idx)
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        _is_multi_selected: bool,
        is_expanded: bool,
    ) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);

        let mut spans = Vec::new();

        if depth > 0 {
            spans.push(Span::styled(indent, Style::default()));
        }

        // Expand/collapse indicator
        if !self.field_mappings.is_empty() {
            let indicator = if is_expanded { "▼ " } else { "▶ " };
            spans.push(Span::styled(indicator, Style::default().fg(theme.text_tertiary)));
        } else {
            spans.push(Span::styled("  ", Style::default()));
        }

        // Priority badge
        spans.push(Span::styled(
            format!("[{}] ", self.priority),
            Style::default().fg(theme.accent_tertiary),
        ));

        // Source entity
        spans.push(Span::styled(
            self.source_entity.clone(),
            Style::default().fg(theme.accent_primary),
        ));

        // Arrow
        spans.push(Span::styled(" → ", Style::default().fg(theme.border_primary)));

        // Target entity
        spans.push(Span::styled(
            self.target_entity.clone(),
            Style::default().fg(theme.accent_secondary),
        ));

        // Field count
        spans.push(Span::styled(
            format!(" ({} fields)", self.field_mappings.len()),
            Style::default().fg(theme.text_tertiary),
        ));

        let mut builder = Element::styled_text(Line::from(spans));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

#[derive(Clone)]
pub struct FieldNode {
    pub entity_idx: usize,
    pub field_idx: usize,
    pub mapping: FieldMapping,
}

impl FieldNode {
    fn id(&self) -> String {
        format!("field_{}_{}", self.entity_idx, self.field_idx)
    }

    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        _is_multi_selected: bool,
        _is_expanded: bool,
    ) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let indent = "  ".repeat(depth);

        let mut spans = Vec::new();

        if depth > 0 {
            spans.push(Span::styled(indent, Style::default()));
        }

        // Target field
        spans.push(Span::styled(
            self.mapping.target_field.clone(),
            Style::default().fg(theme.text_primary),
        ));

        // Transform description
        spans.push(Span::styled(
            format!(" ← {}", self.mapping.transform.describe()),
            Style::default().fg(theme.text_secondary),
        ));

        let mut builder = Element::styled_text(Line::from(spans));

        if is_selected {
            builder = builder.background(Style::default().bg(theme.bg_surface));
        }

        builder.build()
    }
}

/// Build tree items from a TransferConfig
pub fn build_tree(config: &crate::transfer::TransferConfig) -> Vec<MappingTreeItem> {
    let mut items = Vec::new();

    // Add resolvers first
    for (idx, resolver) in config.resolvers.iter().enumerate() {
        items.push(MappingTreeItem::Resolver(ResolverNode::from_resolver(idx, resolver)));
    }

    // Add entity mappings
    for (idx, em) in config.entity_mappings.iter().enumerate() {
        items.push(MappingTreeItem::Entity(EntityNode::from_mapping(idx, em)));
    }

    items
}
