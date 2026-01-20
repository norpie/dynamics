use std::collections::HashMap;

use crate::api::{FieldMetadata, FieldType};
use crate::transfer::{EntityMapping, FieldMapping, Resolver};
use crate::tui::{Element, widgets::TreeItem};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

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
            Self::Entity(node) => !node.resolvers.is_empty() || !node.field_mappings.is_empty(),
            Self::Field(_) => false,
        }
    }

    fn children(&self) -> Vec<Self> {
        match self {
            Self::Resolver(_) => vec![],
            Self::Entity(node) => {
                let mut children = Vec::new();

                // Add resolvers first
                for (idx, resolver) in node.resolvers.iter().enumerate() {
                    children.push(Self::Resolver(ResolverNode::from_resolver(
                        node.idx, idx, resolver,
                    )));
                }

                // Then add field mappings
                for (idx, fm) in node.field_mappings.iter().enumerate() {
                    // Look up field type from cache
                    let field_type = node.field_types.get(&fm.target_field).cloned();
                    children.push(Self::Field(FieldNode {
                        entity_idx: node.idx,
                        field_idx: idx,
                        mapping: fm.clone(),
                        field_type,
                    }));
                }

                children
            }
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
            Self::Resolver(node) => {
                node.to_element(depth, is_selected, is_multi_selected, is_expanded)
            }
            Self::Entity(node) => {
                node.to_element(depth, is_selected, is_multi_selected, is_expanded)
            }
            Self::Field(node) => {
                node.to_element(depth, is_selected, is_multi_selected, is_expanded)
            }
        }
    }
}

/// Node representing a resolver in the tree (child of an entity mapping)
#[derive(Clone)]
pub struct ResolverNode {
    pub entity_idx: usize,
    pub resolver_idx: usize,
    pub name: String,
    pub source_entity: String,
    /// Display string for match fields (e.g., "field1" or "field1, field2")
    pub match_fields_display: String,
}

impl ResolverNode {
    pub fn from_resolver(entity_idx: usize, resolver_idx: usize, resolver: &Resolver) -> Self {
        // Build display string for match fields showing source_path -> target_field
        let match_fields_display = resolver
            .match_fields
            .iter()
            .map(|mf| {
                let source = mf.source_path.to_string();
                if source == mf.target_field {
                    mf.target_field.clone()
                } else {
                    format!("{} → {}", source, mf.target_field)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        Self {
            entity_idx,
            resolver_idx,
            name: resolver.name.clone(),
            source_entity: resolver.source_entity.clone(),
            match_fields_display,
        }
    }

    fn id(&self) -> String {
        format!("resolver_{}_{}", self.entity_idx, self.resolver_idx)
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
        spans.push(Span::styled(
            " → ",
            Style::default().fg(theme.border_primary),
        ));

        // Source entity
        spans.push(Span::styled(
            self.source_entity.clone(),
            Style::default().fg(theme.accent_secondary),
        ));

        // Match field(s) in parentheses
        spans.push(Span::styled(
            format!(" ({})", self.match_fields_display),
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
    pub resolvers: Vec<Resolver>,
    pub field_mappings: Vec<FieldMapping>,
    /// Field name -> field type (for displaying in tree)
    pub field_types: HashMap<String, FieldType>,
}

impl EntityNode {
    pub fn from_mapping(
        idx: usize,
        mapping: &EntityMapping,
        field_metadata: Option<&[FieldMetadata]>,
    ) -> Self {
        // Build field type lookup from metadata if available
        let field_types: HashMap<String, FieldType> = field_metadata
            .map(|fields| {
                fields
                    .iter()
                    .map(|f| (f.logical_name.clone(), f.field_type.clone()))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            idx,
            source_entity: mapping.source_entity.clone(),
            target_entity: mapping.target_entity.clone(),
            priority: mapping.priority,
            resolvers: mapping.resolvers.clone(),
            field_mappings: mapping.field_mappings.clone(),
            field_types,
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
            spans.push(Span::styled(
                indicator,
                Style::default().fg(theme.text_tertiary),
            ));
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
        spans.push(Span::styled(
            " → ",
            Style::default().fg(theme.border_primary),
        ));

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
    pub field_type: Option<FieldType>,
}

impl FieldNode {
    fn id(&self) -> String {
        format!("field_{}_{}", self.entity_idx, self.field_idx)
    }

    fn format_field_type(ft: &FieldType) -> &'static str {
        match ft {
            FieldType::String => "String",
            FieldType::Integer => "Integer",
            FieldType::Decimal => "Decimal",
            FieldType::Boolean => "Boolean",
            FieldType::DateTime => "DateTime",
            FieldType::Lookup => "Lookup",
            FieldType::OptionSet => "OptionSet",
            FieldType::MultiSelectOptionSet => "MultiSelect",
            FieldType::Money => "Money",
            FieldType::Memo => "Memo",
            FieldType::UniqueIdentifier => "GUID",
            FieldType::Other(_) => "Other",
        }
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

        // Field type (if available)
        if let Some(ft) = &self.field_type {
            spans.push(Span::styled(
                format!(" [{}]", Self::format_field_type(ft)),
                Style::default().fg(theme.accent_tertiary),
            ));
        }

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
/// `field_cache` maps entity_idx to field metadata (for showing types in tree)
pub fn build_tree(
    config: &crate::transfer::TransferConfig,
    field_cache: &HashMap<usize, Vec<FieldMetadata>>,
) -> Vec<MappingTreeItem> {
    let mut items = Vec::new();

    // Add entity mappings (resolvers are now children of entities)
    for (idx, em) in config.entity_mappings.iter().enumerate() {
        let field_metadata = field_cache.get(&idx).map(|v| v.as_slice());
        items.push(MappingTreeItem::Entity(EntityNode::from_mapping(
            idx,
            em,
            field_metadata,
        )));
    }

    items
}
