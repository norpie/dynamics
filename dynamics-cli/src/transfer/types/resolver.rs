//! Resolver types for lookup value resolution
//!
//! Resolvers allow transforms to resolve lookup field values by matching
//! a source value against a field in the target environment, instead of
//! directly copying GUIDs.

use serde::{Deserialize, Serialize};

/// A resolver configuration that defines how to match source values
/// to target records for lookup field resolution.
///
/// For example, to resolve a user by email:
/// - `name`: "user_by_email"
/// - `source_entity`: "contact"  (the entity to search in target)
/// - `match_field`: "emailaddress1"  (the field to match against)
/// - `fallback`: what to do when no match is found
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resolver {
    /// Database ID (None if not yet persisted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Unique name for this resolver within the config
    pub name: String,
    /// Target entity to search for matches (logical name)
    pub source_entity: String,
    /// Field to match against in the target entity
    pub match_field: String,
    /// What to do when no match is found
    #[serde(default)]
    pub fallback: ResolverFallback,
}

impl Resolver {
    /// Create a new resolver
    pub fn new(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_field: impl Into<String>,
    ) -> Self {
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_field: match_field.into(),
            fallback: ResolverFallback::default(),
        }
    }

    /// Create a new resolver with custom fallback
    pub fn with_fallback(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_field: impl Into<String>,
        fallback: ResolverFallback,
    ) -> Self {
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_field: match_field.into(),
            fallback,
        }
    }
}

/// Fallback behavior for resolver when no match is found
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverFallback {
    /// Mark the record as an error (won't be transferred)
    #[default]
    Error,
    /// Use null for the lookup field
    Null,
}

impl ResolverFallback {
    /// Get display label for UI
    pub fn label(&self) -> &'static str {
        match self {
            ResolverFallback::Error => "Error",
            ResolverFallback::Null => "Null",
        }
    }

    /// Get all variants for UI selection
    pub fn all_variants() -> &'static [ResolverFallback] {
        &[ResolverFallback::Error, ResolverFallback::Null]
    }

    /// Convert from index (for UI selection)
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => ResolverFallback::Error,
            1 => ResolverFallback::Null,
            _ => ResolverFallback::Error,
        }
    }

    /// Convert to index (for UI selection)
    pub fn to_index(&self) -> usize {
        match self {
            ResolverFallback::Error => 0,
            ResolverFallback::Null => 1,
        }
    }

    /// Cycle to the next fallback option
    pub fn cycle(&self) -> Self {
        match self {
            ResolverFallback::Error => ResolverFallback::Null,
            ResolverFallback::Null => ResolverFallback::Error,
        }
    }
}

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Runtime context for resolver lookups
///
/// Built from target entity data, provides fast lookups to resolve
/// source values to target record GUIDs.
#[derive(Debug, Default)]
pub struct ResolverContext {
    /// Lookup tables: resolver_name -> (normalized_value -> guid)
    tables: HashMap<String, HashMap<String, Uuid>>,
    /// Values that have duplicates (resolver_name -> set of normalized values)
    duplicates: HashMap<String, HashSet<String>>,
}

/// Result of a resolver lookup
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveResult {
    /// Successfully resolved to a GUID
    Found(Uuid),
    /// No match found
    NotFound,
    /// Multiple records have this value (ambiguous)
    Duplicate,
}

impl ResolverContext {
    /// Create a new empty resolver context
    pub fn new() -> Self {
        Self::default()
    }

    /// Build resolver tables from target data
    ///
    /// # Arguments
    /// * `resolvers` - The resolver configurations to build tables for
    /// * `target_data` - Target entity data keyed by entity name
    /// * `primary_keys` - Primary key field names keyed by entity name
    ///
    /// # Returns
    /// A ResolverContext with all lookup tables populated
    pub fn build(
        resolvers: &[Resolver],
        target_data: &HashMap<String, Vec<serde_json::Value>>,
        primary_keys: &HashMap<String, String>,
    ) -> Self {
        let mut ctx = Self::new();

        for resolver in resolvers {
            let Some(records) = target_data.get(&resolver.source_entity) else {
                log::warn!(
                    "Resolver '{}' references entity '{}' which has no data",
                    resolver.name,
                    resolver.source_entity
                );
                continue;
            };

            let Some(pk_field) = primary_keys.get(&resolver.source_entity) else {
                log::warn!(
                    "Resolver '{}' references entity '{}' which has no primary key",
                    resolver.name,
                    resolver.source_entity
                );
                continue;
            };

            let mut table: HashMap<String, Uuid> = HashMap::new();
            let mut duplicates: HashSet<String> = HashSet::new();
            let mut seen: HashSet<String> = HashSet::new();

            for record in records {
                // Get the primary key value
                let Some(pk_value) = record.get(pk_field).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Ok(guid) = Uuid::parse_str(pk_value) else {
                    continue;
                };

                // Get the match field value
                let Some(match_value) = record.get(&resolver.match_field) else {
                    continue;
                };

                // Convert to string and normalize (lowercase for case-insensitive matching)
                let normalized = Self::normalize_value(match_value);
                if normalized.is_empty() {
                    continue;
                }

                // Track duplicates
                if seen.contains(&normalized) {
                    duplicates.insert(normalized.clone());
                    table.remove(&normalized); // Remove from table since it's ambiguous
                } else {
                    seen.insert(normalized.clone());
                    if !duplicates.contains(&normalized) {
                        table.insert(normalized, guid);
                    }
                }
            }

            if !duplicates.is_empty() {
                log::warn!(
                    "Resolver '{}' has {} duplicate values in field '{}'",
                    resolver.name,
                    duplicates.len(),
                    resolver.match_field
                );
            }

            ctx.tables.insert(resolver.name.clone(), table);
            if !duplicates.is_empty() {
                ctx.duplicates.insert(resolver.name.clone(), duplicates);
            }
        }

        ctx
    }

    /// Resolve a value using a specific resolver
    ///
    /// # Arguments
    /// * `resolver_name` - The name of the resolver to use
    /// * `value` - The value to look up
    ///
    /// # Returns
    /// The resolution result (Found, NotFound, or Duplicate)
    pub fn resolve(&self, resolver_name: &str, value: &serde_json::Value) -> ResolveResult {
        let normalized = Self::normalize_value(value);
        if normalized.is_empty() {
            return ResolveResult::NotFound;
        }

        // Check if this value is a duplicate
        if let Some(dups) = self.duplicates.get(resolver_name) {
            if dups.contains(&normalized) {
                return ResolveResult::Duplicate;
            }
        }

        // Look up in the table
        if let Some(table) = self.tables.get(resolver_name) {
            if let Some(guid) = table.get(&normalized) {
                return ResolveResult::Found(*guid);
            }
        }

        ResolveResult::NotFound
    }

    /// Check if a resolver exists in this context
    pub fn has_resolver(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    /// Normalize a JSON value for case-insensitive lookup
    fn normalize_value(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.to_lowercase().trim().to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            _ => String::new(), // Arrays and objects are not supported
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_new() {
        let resolver = Resolver::new("user_by_email", "contact", "emailaddress1");
        assert_eq!(resolver.name, "user_by_email");
        assert_eq!(resolver.source_entity, "contact");
        assert_eq!(resolver.match_field, "emailaddress1");
        assert_eq!(resolver.fallback, ResolverFallback::Error);
        assert_eq!(resolver.id, None);
    }

    #[test]
    fn test_resolver_with_fallback() {
        let resolver =
            Resolver::with_fallback("user_by_email", "contact", "emailaddress1", ResolverFallback::Null);
        assert_eq!(resolver.fallback, ResolverFallback::Null);
    }

    #[test]
    fn test_resolver_fallback_cycle() {
        let fallback = ResolverFallback::Error;
        assert_eq!(fallback.cycle(), ResolverFallback::Null);
        assert_eq!(fallback.cycle().cycle(), ResolverFallback::Error);
    }

    #[test]
    fn test_resolver_serialization() {
        let resolver = Resolver::new("test", "account", "name");
        let json = serde_json::to_string(&resolver).unwrap();
        let deserialized: Resolver = serde_json::from_str(&json).unwrap();
        assert_eq!(resolver, deserialized);
    }

    #[test]
    fn test_resolver_context_build_and_resolve() {
        use serde_json::json;

        let resolvers = vec![Resolver::new("contact_by_email", "contact", "emailaddress1")];

        let mut target_data = HashMap::new();
        target_data.insert(
            "contact".to_string(),
            vec![
                json!({
                    "contactid": "11111111-1111-1111-1111-111111111111",
                    "emailaddress1": "john@example.com"
                }),
                json!({
                    "contactid": "22222222-2222-2222-2222-222222222222",
                    "emailaddress1": "jane@example.com"
                }),
            ],
        );

        let mut primary_keys = HashMap::new();
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Test successful lookup
        let result = ctx.resolve("contact_by_email", &json!("john@example.com"));
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );

        // Test case-insensitive lookup
        let result = ctx.resolve("contact_by_email", &json!("JOHN@EXAMPLE.COM"));
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );

        // Test not found
        let result = ctx.resolve("contact_by_email", &json!("unknown@example.com"));
        assert_eq!(result, ResolveResult::NotFound);

        // Test null value
        let result = ctx.resolve("contact_by_email", &serde_json::Value::Null);
        assert_eq!(result, ResolveResult::NotFound);
    }

    #[test]
    fn test_resolver_context_duplicates() {
        use serde_json::json;

        let resolvers = vec![Resolver::new("contact_by_email", "contact", "emailaddress1")];

        let mut target_data = HashMap::new();
        target_data.insert(
            "contact".to_string(),
            vec![
                json!({
                    "contactid": "11111111-1111-1111-1111-111111111111",
                    "emailaddress1": "duplicate@example.com"
                }),
                json!({
                    "contactid": "22222222-2222-2222-2222-222222222222",
                    "emailaddress1": "duplicate@example.com"  // Same email!
                }),
                json!({
                    "contactid": "33333333-3333-3333-3333-333333333333",
                    "emailaddress1": "unique@example.com"
                }),
            ],
        );

        let mut primary_keys = HashMap::new();
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Duplicate should return Duplicate result
        let result = ctx.resolve("contact_by_email", &json!("duplicate@example.com"));
        assert_eq!(result, ResolveResult::Duplicate);

        // Unique should still work
        let result = ctx.resolve("contact_by_email", &json!("unique@example.com"));
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap())
        );
    }

    #[test]
    fn test_resolver_context_missing_entity() {
        let resolvers = vec![Resolver::new("contact_by_email", "contact", "emailaddress1")];
        let target_data = HashMap::new(); // No data
        let primary_keys = HashMap::new();

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Should return NotFound for unknown resolver
        let result = ctx.resolve("contact_by_email", &serde_json::json!("test@example.com"));
        assert_eq!(result, ResolveResult::NotFound);
    }

    #[test]
    fn test_resolver_context_numeric_values() {
        use serde_json::json;

        let resolvers = vec![Resolver::new("account_by_code", "account", "accountnumber")];

        let mut target_data = HashMap::new();
        target_data.insert(
            "account".to_string(),
            vec![json!({
                "accountid": "11111111-1111-1111-1111-111111111111",
                "accountnumber": 12345
            })],
        );

        let mut primary_keys = HashMap::new();
        primary_keys.insert("account".to_string(), "accountid".to_string());

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Should work with numeric values
        let result = ctx.resolve("account_by_code", &json!(12345));
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );
    }
}
