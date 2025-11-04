// Matching service for computing field/relationship/entity mappings
//
// This service provides pure business logic for matching operations,
// decoupled from the TUI and reusable across different contexts.

pub mod core;
pub mod models;

// Re-export commonly used types
pub use models::{MatchInfo, MatchType};

use crate::api::metadata::EntityMetadata;
use std::collections::{HashMap, HashSet};

/// Input context for matching operations
#[derive(Debug, Clone)]
pub struct MatchingContext {
    pub source_metadata: EntityMetadata,
    pub target_metadata: EntityMetadata,
    pub source_entity: String,
    pub target_entity: String,
}

/// Mapping configuration for matching
#[derive(Debug, Clone)]
pub struct MatchingMappings {
    pub field_mappings: HashMap<String, Vec<String>>,
    pub prefix_mappings: HashMap<String, Vec<String>>,
    pub imported_mappings: HashMap<String, Vec<String>>,
    pub negative_matches: HashSet<String>,
}

/// Complete matching results
#[derive(Debug, Clone)]
pub struct MatchingResults {
    pub field_matches: HashMap<String, MatchInfo>,
    pub relationship_matches: HashMap<String, MatchInfo>,
    pub entity_matches: HashMap<String, MatchInfo>,
    pub source_entities: Vec<(String, usize)>,
    pub target_entities: Vec<(String, usize)>,
}

/// Compute all matches between source and target entities
/// Main orchestrator function for the matching service
pub fn compute_all_matches(
    context: &MatchingContext,
    mappings: &MatchingMappings,
) -> MatchingResults {
    // Compute flat field matches (Fields tab)
    let field_matches = core::compute_field_matches(
        &context.source_metadata.fields,
        &context.target_metadata.fields,
        &mappings.field_mappings,
        &mappings.imported_mappings,
        &mappings.prefix_mappings,
        &mappings.negative_matches,
    );

    // Extract entities from relationships
    let source_entities = core::extract_entities(&context.source_metadata.relationships);
    let target_entities = core::extract_entities(&context.target_metadata.relationships);

    // Compute entity matches (uses same mappings as fields)
    let entity_matches = core::compute_entity_matches(
        &source_entities,
        &target_entities,
        &mappings.field_mappings,
        &mappings.prefix_mappings,
    );

    // Compute relationship matches (entity-aware)
    let relationship_matches = core::compute_relationship_matches(
        &context.source_metadata.relationships,
        &context.target_metadata.relationships,
        &mappings.field_mappings,
        &mappings.prefix_mappings,
        &entity_matches,
    );

    MatchingResults {
        field_matches,
        relationship_matches,
        entity_matches,
        source_entities,
        target_entities,
    }
}
