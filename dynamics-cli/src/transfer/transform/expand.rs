//! OData expand clause generation for nested lookup paths
//!
//! Handles building nested $expand clauses from FieldPaths.
//! For example, paths like `a.b.c` and `a.d` become:
//! `$expand=a($select=d;$expand=b($select=c))`

use std::collections::{BTreeMap, BTreeSet};

use crate::transfer::types::{FieldPath, Transform};

/// Case-insensitive lookup in a HashMap - returns the VALUE (navigation property name)
fn get_case_insensitive<'a>(
    map: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Option<&'a String> {
    let key_lower = key.to_lowercase();
    map.iter()
        .find(|(k, _)| k.to_lowercase() == key_lower)
        .map(|(_, v)| v)
}

/// Case-insensitive contains check in a HashSet
fn contains_case_insensitive(set: &std::collections::HashSet<String>, key: &str) -> bool {
    let key_lower = key.to_lowercase();
    set.iter().any(|k| k.to_lowercase() == key_lower)
}

/// Case-insensitive contains check in a BTreeMap
fn btree_contains_key_case_insensitive<V>(map: &BTreeMap<String, V>, key: &str) -> bool {
    let key_lower = key.to_lowercase();
    map.keys().any(|k| k.to_lowercase() == key_lower)
}

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
    ///
    /// # Arguments
    /// * `nav_prop_map` - Optional map from logical name to navigation property name (schema name)
    ///   Used to convert lookup field names to proper OData navigation property names.
    ///   If None or if a field is not in the map, the logical name is used as-is.
    /// * `lookup_fields` - Optional set of field names that are lookup fields across all related entities.
    ///   When selecting these fields, they need to be prefixed with `_` and suffixed with `_value`.
    pub fn build_expand_clauses(
        &self,
        nav_prop_map: Option<&std::collections::HashMap<String, String>>,
        lookup_fields: Option<&std::collections::HashSet<String>>,
    ) -> Vec<String> {
        self.nodes
            .iter()
            .map(|(field, node)| {
                let nav_prop_name = nav_prop_map
                    .and_then(|m| get_case_insensitive(m, field))
                    .map(|s| s.as_str())
                    .unwrap_or(field);
                Self::build_node_string(nav_prop_name, field, node, nav_prop_map, lookup_fields)
            })
            .collect()
    }

    /// Build the expand string for a single node
    ///
    /// # Arguments
    /// * `nav_prop_name` - The navigation property name to use in the output (may be schema-cased)
    /// * `logical_name` - The logical name of the field (used for intermediate selects)
    /// * `node` - The expand node
    /// * `nav_prop_map` - Optional map for converting child field names
    /// * `lookup_fields` - Optional set of lookup field names that need _value format
    fn build_node_string(
        nav_prop_name: &str,
        logical_name: &str,
        node: &ExpandNode,
        nav_prop_map: Option<&std::collections::HashMap<String, String>>,
        lookup_fields: Option<&std::collections::HashSet<String>>,
    ) -> String {
        let mut parts = Vec::new();

        // Build select fields, converting lookup fields to OData _value format
        // A field needs _value format if:
        // 1. It's also being expanded (in children), OR
        // 2. It's in the lookup_fields set (known to be a lookup on a related entity)
        let select_fields: Vec<String> = node
            .selects
            .iter()
            .map(|s| {
                let is_expanded = btree_contains_key_case_insensitive(&node.children, s);
                let is_known_lookup = lookup_fields
                    .map(|lf| contains_case_insensitive(lf, s))
                    .unwrap_or(false);

                if is_expanded || is_known_lookup {
                    // This field is a lookup - use _value format for the GUID
                    format!("_{}_value", s)
                } else {
                    s.clone()
                }
            })
            .collect();

        // Add $select for leaf fields OR use the logical name to minimize intermediate level data
        if !select_fields.is_empty() {
            let fields: Vec<&str> = select_fields.iter().map(|s| s.as_str()).collect();
            parts.push(format!("$select={}", fields.join(",")));
        } else if !node.children.is_empty() {
            // Intermediate node with no direct selects - add $select={logical_name} to minimize returned fields
            parts.push(format!("$select={}", logical_name));
        }

        // Add nested $expand if there are children
        if !node.children.is_empty() {
            let nested: Vec<String> = node
                .children
                .iter()
                .map(|(child_field, child_node)| {
                    let child_nav_prop = nav_prop_map
                        .and_then(|m| get_case_insensitive(m, child_field))
                        .map(|s| s.as_str())
                        .unwrap_or(child_field);
                    Self::build_node_string(
                        child_nav_prop,
                        child_field,
                        child_node,
                        nav_prop_map,
                        lookup_fields,
                    )
                })
                .collect();
            parts.push(format!("$expand={}", nested.join(",")));
        }

        if parts.is_empty() {
            nav_prop_name.to_string()
        } else {
            format!("{}({})", nav_prop_name, parts.join(";"))
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
        assert!(tree.build_expand_clauses(None, None).is_empty());
    }

    #[test]
    fn test_single_lookup() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("accountid.name").unwrap());

        let clauses = tree.build_expand_clauses(None, None);
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0], "accountid($select=name)");
    }

    #[test]
    fn test_single_lookup_multiple_fields() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("accountid.name").unwrap());
        tree.add_path(&FieldPath::parse("accountid.revenue").unwrap());

        let clauses = tree.build_expand_clauses(None, None);
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0], "accountid($select=name,revenue)");
    }

    #[test]
    fn test_two_level_nested() {
        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("userid.contactid.emailaddress1").unwrap());

        let clauses = tree.build_expand_clauses(None, None);
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
        tree.add_path(&FieldPath::parse("userid.contactid.parentcustomerid_account.name").unwrap());

        let clauses = tree.build_expand_clauses(None, None);
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

        let clauses = tree.build_expand_clauses(None, None);
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

        let clauses = tree.build_expand_clauses(None, None);
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

        let clauses = tree.build_expand_clauses(None, None);
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

        let clauses = tree.build_expand_clauses(None, None);
        assert_eq!(clauses.len(), 1);
        // projectmanagerid is both selected AND expanded, so select uses _value format
        assert_eq!(
            clauses[0],
            "deadlineid($select=_projectmanagerid_value;$expand=projectmanagerid($select=email))"
        );
    }

    #[test]
    fn test_nav_prop_map_converts_names() {
        use std::collections::HashMap;

        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("nrq_deadlineid.nrq_typeid").unwrap());

        // Without nav prop map - uses logical names
        let clauses = tree.build_expand_clauses(None, None);
        assert_eq!(clauses[0], "nrq_deadlineid($select=nrq_typeid)");

        // With nav prop map - converts to schema names
        let mut nav_prop_map = HashMap::new();
        nav_prop_map.insert("nrq_deadlineid".to_string(), "nrq_DeadlineId".to_string());

        let clauses = tree.build_expand_clauses(Some(&nav_prop_map), None);
        assert_eq!(clauses[0], "nrq_DeadlineId($select=nrq_typeid)");
    }

    #[test]
    fn test_nav_prop_map_nested() {
        use std::collections::HashMap;

        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("userid.contactid.email").unwrap());

        let mut nav_prop_map = HashMap::new();
        nav_prop_map.insert("userid".to_string(), "UserId".to_string());
        nav_prop_map.insert("contactid".to_string(), "ContactId".to_string());

        let clauses = tree.build_expand_clauses(Some(&nav_prop_map), None);
        // Both navigation properties should be converted
        assert_eq!(
            clauses[0],
            "UserId($select=userid;$expand=ContactId($select=email))"
        );
    }

    #[test]
    fn test_lookup_fields_converts_to_value_format() {
        use std::collections::HashSet;

        let mut tree = ExpandTree::new();
        // nrq_typeid is a lookup field on the related entity
        tree.add_path(&FieldPath::parse("nrq_deadlineid.nrq_typeid").unwrap());

        // Without lookup_fields - uses field name as-is
        let clauses = tree.build_expand_clauses(None, None);
        assert_eq!(clauses[0], "nrq_deadlineid($select=nrq_typeid)");

        // With lookup_fields - converts to _value format
        let mut lookup_fields = HashSet::new();
        lookup_fields.insert("nrq_typeid".to_string());

        let clauses = tree.build_expand_clauses(None, Some(&lookup_fields));
        assert_eq!(clauses[0], "nrq_deadlineid($select=_nrq_typeid_value)");
    }

    #[test]
    fn test_both_nav_prop_and_lookup_fields() {
        use std::collections::HashMap;
        use std::collections::HashSet;

        let mut tree = ExpandTree::new();
        tree.add_path(&FieldPath::parse("nrq_deadlineid.nrq_typeid").unwrap());

        let mut nav_prop_map = HashMap::new();
        nav_prop_map.insert("nrq_deadlineid".to_string(), "nrq_DeadlineId".to_string());

        let mut lookup_fields = HashSet::new();
        lookup_fields.insert("nrq_typeid".to_string());

        let clauses = tree.build_expand_clauses(Some(&nav_prop_map), Some(&lookup_fields));
        // Navigation property should be schema-cased, and lookup field should use _value format
        assert_eq!(clauses[0], "nrq_DeadlineId($select=_nrq_typeid_value)");
    }

    #[test]
    fn test_case_insensitive_nav_prop_lookup() {
        use std::collections::HashMap;

        let mut tree = ExpandTree::new();
        // Path uses PascalCase
        tree.add_path(&FieldPath::parse("OwnerId.domainname").unwrap());

        // Map has lowercase key
        let mut nav_prop_map = HashMap::new();
        nav_prop_map.insert("ownerid".to_string(), "ownerid".to_string());

        let clauses = tree.build_expand_clauses(Some(&nav_prop_map), None);
        // Should find the nav prop despite case mismatch
        assert_eq!(clauses[0], "ownerid($select=domainname)");
    }

    #[test]
    fn test_case_insensitive_lookup_fields() {
        use std::collections::HashSet;

        let mut tree = ExpandTree::new();
        // Path uses lowercase
        tree.add_path(&FieldPath::parse("accountid.primarycontactid").unwrap());

        // Set has PascalCase
        let mut lookup_fields = HashSet::new();
        lookup_fields.insert("PrimaryContactId".to_string());

        let clauses = tree.build_expand_clauses(None, Some(&lookup_fields));
        // Should detect as lookup field despite case mismatch
        assert_eq!(clauses[0], "accountid($select=_primarycontactid_value)");
    }
}
