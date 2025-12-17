//! OData expand clause generation for nested lookup paths
//!
//! Handles building nested $expand clauses from FieldPaths.
//! For example, paths like `a.b.c` and `a.d` become:
//! `$expand=a($select=d;$expand=b($select=c))`

use std::collections::{BTreeMap, BTreeSet};

use crate::transfer::types::{FieldPath, Transform};

/// A tree structure for building nested OData expand clauses
#[derive(Debug, Default)]
pub struct ExpandTree {
    /// Root-level expand nodes, keyed by lookup field name
    nodes: BTreeMap<String, ExpandNode>,
}

/// A node in the expand tree representing a single lookup traversal
#[derive(Debug, Default)]
struct ExpandNode {
    /// Fields to select at this level (leaf values from paths ending here)
    selects: BTreeSet<String>,
    /// Nested expands (key = lookup field name, value = deeper expand)
    children: BTreeMap<String, ExpandNode>,
}

impl ExpandTree {
    /// Create a new empty expand tree
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field path to the expand tree
    ///
    /// For simple paths (no lookup), this does nothing.
    /// For lookup paths like `a.b` or `a.b.c.d`, adds appropriate expand nodes.
    pub fn add_path(&mut self, path: &FieldPath) {
        if !path.is_lookup_traversal() {
            return;
        }

        let segments = path.segments();
        let lookup_segments = &segments[..segments.len() - 1];
        let target_field = path.target_field();

        // Use recursive helper to navigate/create the tree structure
        Self::add_to_node_map(&mut self.nodes, lookup_segments, target_field);
    }

    /// Recursively add a path to a node map
    fn add_to_node_map(
        nodes: &mut BTreeMap<String, ExpandNode>,
        lookup_segments: &[String],
        target_field: &str,
    ) {
        if lookup_segments.is_empty() {
            return;
        }

        let node = nodes.entry(lookup_segments[0].clone()).or_default();

        if lookup_segments.len() == 1 {
            // Last lookup segment - add the target field to selects
            node.selects.insert(target_field.to_string());
        } else {
            // Intermediate segment - recurse into children
            Self::add_to_node_map(&mut node.children, &lookup_segments[1..], target_field);
        }
    }

    /// Build the OData expand clauses as a vector of strings
    ///
    /// Each string is a complete expand clause for a root-level lookup,
    /// e.g., `accountid($select=name;$expand=parentcustomerid($select=name))`
    pub fn build_expand_clauses(&self) -> Vec<String> {
        self.nodes
            .iter()
            .map(|(field, node)| Self::build_node_string(field, node))
            .collect()
    }

    /// Build the expand string for a single node
    fn build_node_string(field: &str, node: &ExpandNode) -> String {
        let mut parts = Vec::new();

        // Build select fields, converting lookup fields to OData _value format
        // A field that's also in children is a lookup (navigation property) - use _fieldname_value
        let select_fields: Vec<String> = node.selects.iter()
            .map(|s| {
                if node.children.contains_key(s) {
                    // This field is also being expanded - it's a lookup, use _value format for the GUID
                    format!("_{}_value", s)
                } else {
                    s.clone()
                }
            })
            .collect();

        // Add $select for leaf fields OR use the field name as PK to minimize intermediate level data
        if !select_fields.is_empty() {
            let fields: Vec<&str> = select_fields.iter().map(|s| s.as_str()).collect();
            parts.push(format!("$select={}", fields.join(",")));
        } else if !node.children.is_empty() {
            // Intermediate node with no direct selects - add $select={field} to minimize returned fields
            // The field name (navigation property) usually matches the target entity's primary key
            parts.push(format!("$select={}", field));
        }

        // Add nested $expand if there are children
        if !node.children.is_empty() {
            let nested: Vec<String> = node
                .children
                .iter()
                .map(|(child_field, child_node)| Self::build_node_string(child_field, child_node))
                .collect();
            parts.push(format!("$expand={}", nested.join(",")));
        }

        if parts.is_empty() {
            field.to_string()
        } else {
            format!("{}({})", field, parts.join(";"))
        }
    }

    /// Check if the tree is empty (no expand clauses needed)
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Add all lookup paths from a transform to the expand tree
    pub fn add_transform(&mut self, transform: &Transform) {
        for path in transform.lookup_paths() {
            self.add_path(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_field_no_expand() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("name").unwrap());
        assert!(tree.is_empty());
        assert!(tree.build_expand_clauses().is_empty());
    }

    #[test]
    fn test_single_lookup() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("accountid.name").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0], "accountid($select=name)");
    }

    #[test]
    fn test_single_lookup_multiple_fields() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("accountid.name").unwrap());
        tree.add_path(&FieldPath::parse("accountid.revenue").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0], "accountid($select=name,revenue)");
    }

    #[test]
    fn test_two_level_nested() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("userid.contactid.emailaddress1").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 1);
        // Intermediate level gets $select={field} to minimize data returned
        assert_eq!(
            clauses[0],
            "userid($select=userid;$expand=contactid($select=emailaddress1))"
        );
    }

    #[test]
    fn test_three_level_nested() {
        let mut tree = ExpandTree::new();
        tree.add_path(
            &FieldPath::parse("userid.contactid.parentcustomerid_account.name").unwrap(),
        );

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 1);
        // All intermediate levels get $select={field} to minimize data
        assert_eq!(
            clauses[0],
            "userid($select=userid;$expand=contactid($select=contactid;$expand=parentcustomerid_account($select=name)))"
        );
    }

    #[test]
    fn test_mixed_depths() {
        let mut tree = ExpandTree::new();
        // Two-level path
        tree.add_path(&FieldPath::parse("userid.contactid.email").unwrap());
        // Direct field from same root lookup
        tree.add_path(&FieldPath::parse("userid.fullname").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 1);
        // Should have both select and expand
        assert_eq!(
            clauses[0],
            "userid($select=fullname;$expand=contactid($select=email))"
        );
    }

    #[test]
    fn test_multiple_root_lookups() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("accountid.name").unwrap());
        tree.add_path(&FieldPath::parse("ownerid.fullname").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 2);
        // BTreeMap ensures consistent order
        assert_eq!(clauses[0], "accountid($select=name)");
        assert_eq!(clauses[1], "ownerid($select=fullname)");
    }

    #[test]
    fn test_complex_tree() {
        let mut tree = ExpandTree::new();
        // Multiple paths through same lookup
        tree.add_path(&FieldPath::parse("userid.email").unwrap());
        tree.add_path(&FieldPath::parse("userid.contactid.firstname").unwrap());
        tree.add_path(&FieldPath::parse("userid.contactid.lastname").unwrap());
        // Different root lookup
        tree.add_path(&FieldPath::parse("accountid.name").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 2);
        assert_eq!(clauses[0], "accountid($select=name)");
        // contactid is NOT in selects, only in children, so no _value conversion needed
        assert_eq!(
            clauses[1],
            "userid($select=email;$expand=contactid($select=firstname,lastname))"
        );
    }

    #[test]
    fn test_lookup_field_both_selected_and_expanded() {
        let mut tree = ExpandTree::new();
        // Path 1: wants the GUID of projectmanagerid (adds to selects)
        tree.add_path(&FieldPath::parse("deadlineid.projectmanagerid").unwrap());
        // Path 2: wants to traverse into projectmanagerid (adds to children)
        tree.add_path(&FieldPath::parse("deadlineid.projectmanagerid.email").unwrap());

        let clauses = tree.build_expand_clauses();
        assert_eq!(clauses.len(), 1);
        // projectmanagerid is both selected AND expanded, so select uses _value format
        assert_eq!(
            clauses[0],
            "deadlineid($select=_projectmanagerid_value;$expand=projectmanagerid($select=email))"
        );
    }
}
