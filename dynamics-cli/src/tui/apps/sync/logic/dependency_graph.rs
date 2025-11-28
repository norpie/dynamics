//! Dependency graph logic for ordering entity operations
//!
//! This module provides functions to:
//! - Build a dependency graph from entity lookup relationships
//! - Perform topological sort for correct insert/delete ordering
//! - Categorize entities as standalone, dependent, or junction
//! - Assign operation priorities

use std::collections::{HashMap, HashSet, VecDeque};

use crate::api::metadata::{FieldMetadata, FieldType};
use crate::tui::apps::sync::types::{
    DependencyCategory, LookupInfo, SyncEntityInfo,
};

/// Entity with its lookup relationships
#[derive(Debug, Clone)]
pub struct EntityWithLookups {
    pub logical_name: String,
    pub display_name: Option<String>,
    pub lookups: Vec<LookupInfo>,
}

impl EntityWithLookups {
    /// Create from entity name and field metadata
    pub fn from_fields(
        logical_name: &str,
        display_name: Option<&str>,
        fields: &[FieldMetadata],
        selected_entities: &HashSet<String>,
    ) -> Self {
        let lookups = fields
            .iter()
            .filter(|f| matches!(f.field_type, FieldType::Lookup))
            .filter_map(|f| {
                f.related_entity.as_ref().map(|target| LookupInfo {
                    field_name: f.logical_name.clone(),
                    target_entity: target.clone(),
                    is_internal: selected_entities.contains(target),
                })
            })
            .collect();

        Self {
            logical_name: logical_name.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            lookups,
        }
    }

    /// Get internal lookups (to other selected entities)
    pub fn internal_lookups(&self) -> Vec<&LookupInfo> {
        self.lookups.iter().filter(|l| l.is_internal).collect()
    }

    /// Get external lookups (to entities not in selection)
    pub fn external_lookups(&self) -> Vec<&LookupInfo> {
        self.lookups.iter().filter(|l| !l.is_internal).collect()
    }

    /// Count of unique internal lookup targets (excluding self-references)
    pub fn internal_target_count(&self) -> usize {
        let targets: HashSet<_> = self.internal_lookups()
            .iter()
            .map(|l| &l.target_entity)
            // Exclude self-references
            .filter(|t| *t != &self.logical_name)
            .collect();
        targets.len()
    }
}

/// Dependency graph for a set of entities
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// All entities in the graph
    pub entities: HashMap<String, EntityWithLookups>,
    /// Adjacency list: entity -> entities it depends on (has lookups to)
    pub dependencies: HashMap<String, HashSet<String>>,
    /// Reverse adjacency: entity -> entities that depend on it
    pub dependents: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    /// Build a dependency graph from entities with their fields
    pub fn build(
        entities_with_fields: Vec<(String, Option<String>, Vec<FieldMetadata>)>,
    ) -> Self {
        let selected_set: HashSet<String> = entities_with_fields
            .iter()
            .map(|(name, _, _)| name.clone())
            .collect();

        let mut graph = DependencyGraph::default();

        // Build entity lookup info
        for (name, display_name, fields) in &entities_with_fields {
            let entity = EntityWithLookups::from_fields(
                name,
                display_name.as_deref(),
                fields,
                &selected_set,
            );
            graph.entities.insert(name.clone(), entity);
        }

        // Build adjacency lists
        for (name, entity) in &graph.entities {
            let deps: HashSet<String> = entity
                .internal_lookups()
                .iter()
                .map(|l| l.target_entity.clone())
                // Don't include self-references
                .filter(|t| t != name)
                .collect();

            graph.dependencies.insert(name.clone(), deps.clone());

            // Update reverse adjacency
            for dep in deps {
                graph.dependents
                    .entry(dep)
                    .or_default()
                    .insert(name.clone());
            }
        }

        // Ensure all entities have entries in dependents map
        for name in graph.entities.keys() {
            graph.dependents.entry(name.clone()).or_default();
        }

        graph
    }

    /// Categorize an entity based on its lookup relationships
    pub fn categorize(&self, entity_name: &str) -> DependencyCategory {
        let entity = match self.entities.get(entity_name) {
            Some(e) => e,
            None => return DependencyCategory::Standalone,
        };

        let internal_target_count = entity.internal_target_count();

        if internal_target_count >= 2 {
            DependencyCategory::Junction
        } else if internal_target_count == 1 {
            DependencyCategory::Dependent
        } else {
            DependencyCategory::Standalone
        }
    }

    /// Perform topological sort using Kahn's algorithm
    /// Returns entities in insert order (dependencies first)
    pub fn topological_sort(&self) -> Result<Vec<String>, CycleError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // Calculate in-degrees
        for name in self.entities.keys() {
            in_degree.insert(name.clone(), 0);
        }
        for deps in self.dependencies.values() {
            for dep in deps {
                if let Some(count) = in_degree.get_mut(dep) {
                    *count += 1;
                }
            }
        }

        // Actually, we need to reverse this - we want entities with NO dependencies first
        // Let me recalculate: in_degree should be count of dependencies (outgoing), not dependents
        in_degree.clear();
        for (name, deps) in &self.dependencies {
            in_degree.insert(name.clone(), deps.len());
        }

        // Start with entities that have no dependencies
        for (name, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(name.clone());
            }
        }

        while let Some(entity) = queue.pop_front() {
            result.push(entity.clone());

            // For each entity that depends on this one, decrease their in-degree
            if let Some(dependents) = self.dependents.get(&entity) {
                for dependent in dependents {
                    if let Some(count) = in_degree.get_mut(dependent) {
                        *count -= 1;
                        if *count == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != self.entities.len() {
            let remaining: Vec<_> = self.entities.keys()
                .filter(|e| !result.contains(e))
                .cloned()
                .collect();
            return Err(CycleError { entities: remaining });
        }

        Ok(result)
    }

    /// Get insert order (dependencies before dependents)
    pub fn insert_order(&self) -> Result<Vec<String>, CycleError> {
        self.topological_sort()
    }

    /// Get delete order (dependents before dependencies - reverse of insert)
    pub fn delete_order(&self) -> Result<Vec<String>, CycleError> {
        let mut order = self.topological_sort()?;
        order.reverse();
        Ok(order)
    }

    /// Build SyncEntityInfo for all entities with assigned priorities
    pub fn build_entity_infos(&self) -> Result<Vec<SyncEntityInfo>, CycleError> {
        let insert_order = self.insert_order()?;

        let mut infos = Vec::new();

        for (priority, entity_name) in insert_order.iter().enumerate() {
            let entity = match self.entities.get(entity_name) {
                Some(e) => e,
                None => continue,
            };

            let dependents: Vec<String> = self.dependents
                .get(entity_name)
                .map(|d| d.iter().cloned().collect())
                .unwrap_or_default();

            let info = SyncEntityInfo {
                logical_name: entity_name.clone(),
                display_name: entity.display_name.clone(),
                primary_name_attribute: None, // Set by caller
                category: self.categorize(entity_name),
                lookups: entity.lookups.clone(),
                incoming_references: vec![], // Set by caller
                dependents,
                insert_priority: priority as u32,
                delete_priority: (insert_order.len() - 1 - priority) as u32,
            };

            infos.push(info);
        }

        Ok(infos)
    }
}

/// Error when a cycle is detected in the dependency graph
#[derive(Debug, Clone)]
pub struct CycleError {
    pub entities: Vec<String>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Circular dependency detected involving: {}", self.entities.join(", "))
    }
}

impl std::error::Error for CycleError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lookup(name: &str, target: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            display_name: Some(name.to_string()),
            field_type: FieldType::Lookup,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: Some(target.to_string()),
        }
    }

    fn make_string_field(name: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            display_name: Some(name.to_string()),
            field_type: FieldType::String,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: None,
        }
    }

    #[test]
    fn test_standalone_entity() {
        let entities = vec![
            ("standalone".to_string(), None, vec![make_string_field("name")]),
        ];

        let graph = DependencyGraph::build(entities);

        assert_eq!(graph.categorize("standalone"), DependencyCategory::Standalone);
    }

    #[test]
    fn test_dependent_entity() {
        let entities = vec![
            ("parent".to_string(), None, vec![make_string_field("name")]),
            ("child".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("parentid", "parent"),
            ]),
        ];

        let graph = DependencyGraph::build(entities);

        assert_eq!(graph.categorize("parent"), DependencyCategory::Standalone);
        assert_eq!(graph.categorize("child"), DependencyCategory::Dependent);
    }

    #[test]
    fn test_junction_entity() {
        let entities = vec![
            ("account".to_string(), None, vec![make_string_field("name")]),
            ("contact".to_string(), None, vec![make_string_field("name")]),
            ("account_contact".to_string(), None, vec![
                make_lookup("accountid", "account"),
                make_lookup("contactid", "contact"),
            ]),
        ];

        let graph = DependencyGraph::build(entities);

        assert_eq!(graph.categorize("account"), DependencyCategory::Standalone);
        assert_eq!(graph.categorize("contact"), DependencyCategory::Standalone);
        assert_eq!(graph.categorize("account_contact"), DependencyCategory::Junction);
    }

    #[test]
    fn test_topological_sort_simple() {
        let entities = vec![
            ("parent".to_string(), None, vec![make_string_field("name")]),
            ("child".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("parentid", "parent"),
            ]),
        ];

        let graph = DependencyGraph::build(entities);
        let order = graph.insert_order().unwrap();

        // Parent should come before child
        let parent_pos = order.iter().position(|e| e == "parent").unwrap();
        let child_pos = order.iter().position(|e| e == "child").unwrap();
        assert!(parent_pos < child_pos);
    }

    #[test]
    fn test_topological_sort_chain() {
        let entities = vec![
            ("grandparent".to_string(), None, vec![make_string_field("name")]),
            ("parent".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("grandparentid", "grandparent"),
            ]),
            ("child".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("parentid", "parent"),
            ]),
        ];

        let graph = DependencyGraph::build(entities);
        let order = graph.insert_order().unwrap();

        let gp_pos = order.iter().position(|e| e == "grandparent").unwrap();
        let p_pos = order.iter().position(|e| e == "parent").unwrap();
        let c_pos = order.iter().position(|e| e == "child").unwrap();

        assert!(gp_pos < p_pos);
        assert!(p_pos < c_pos);
    }

    #[test]
    fn test_delete_order_reverses_insert() {
        let entities = vec![
            ("parent".to_string(), None, vec![make_string_field("name")]),
            ("child".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("parentid", "parent"),
            ]),
        ];

        let graph = DependencyGraph::build(entities);
        let insert = graph.insert_order().unwrap();
        let delete = graph.delete_order().unwrap();

        // Delete order should be reverse of insert
        let mut reversed_insert = insert.clone();
        reversed_insert.reverse();
        assert_eq!(delete, reversed_insert);

        // Child should be deleted before parent
        let parent_pos = delete.iter().position(|e| e == "parent").unwrap();
        let child_pos = delete.iter().position(|e| e == "child").unwrap();
        assert!(child_pos < parent_pos);
    }

    #[test]
    fn test_external_lookup_ignored_for_ordering() {
        let entities = vec![
            ("myentity".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("systemuserid", "systemuser"), // External - not in selection
            ]),
        ];

        let graph = DependencyGraph::build(entities);

        // Should be standalone since systemuser is not in selection
        assert_eq!(graph.categorize("myentity"), DependencyCategory::Standalone);

        // External lookup should be tracked
        let entity = graph.entities.get("myentity").unwrap();
        assert_eq!(entity.external_lookups().len(), 1);
        assert_eq!(entity.internal_lookups().len(), 0);
    }

    #[test]
    fn test_self_reference_ignored() {
        let entities = vec![
            ("account".to_string(), None, vec![
                make_string_field("name"),
                make_lookup("parentaccountid", "account"), // Self-reference
            ]),
        ];

        let graph = DependencyGraph::build(entities);

        // Should be standalone - self-reference doesn't create dependency
        assert_eq!(graph.categorize("account"), DependencyCategory::Standalone);

        // Should still complete topological sort
        let order = graph.insert_order().unwrap();
        assert_eq!(order.len(), 1);
    }
}
