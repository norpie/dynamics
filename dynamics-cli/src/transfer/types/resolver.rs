//! Resolver types for lookup value resolution
//!
//! Resolvers allow transforms to resolve lookup field values by matching
//! a source value against a field in the target environment, instead of
//! directly copying GUIDs.
//!
//! Supports both single-field and compound key matching:
//! - Single field: Match by email, account number, etc.
//! - Compound key: Match by multiple fields (e.g., contactid + requestid)

use serde::{Deserialize, Serialize};

use super::FieldPath;

/// A single match field definition for compound key resolution.
///
/// Each match field specifies:
/// - Where to get the value from the source record (`source_path`)
/// - Which field to match against in the target entity (`target_field`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchField {
    /// Path to extract value from source record (supports nested lookups)
    pub source_path: FieldPath,
    /// Field name to match against in target entity
    pub target_field: String,
}

impl MatchField {
    /// Create a new match field
    pub fn new(source_path: FieldPath, target_field: impl Into<String>) -> Self {
        MatchField {
            source_path,
            target_field: target_field.into(),
        }
    }

    /// Create a simple match field where source and target field names are the same
    pub fn simple(field: impl Into<String>) -> Self {
        let field = field.into();
        MatchField {
            source_path: FieldPath::simple(&field),
            target_field: field,
        }
    }

    /// Create a match field from source path string and target field
    pub fn from_paths(
        source_path: impl AsRef<str>,
        target_field: impl Into<String>,
    ) -> Result<Self, super::FieldPathError> {
        Ok(MatchField {
            source_path: FieldPath::parse(source_path.as_ref())?,
            target_field: target_field.into(),
        })
    }
}

/// A resolver configuration that defines how to match source values
/// to target records for lookup field resolution.
///
/// Supports both single-field and compound key matching:
///
/// # Single Field Example
/// ```ignore
/// // Resolve user by email
/// Resolver::new("user_by_email", "contact", "emailaddress1")
/// ```
///
/// # Compound Key Example
/// ```ignore
/// // Resolve capacity by both contact and request
/// Resolver::compound("capacity_by_pair", "capacity", vec![
///     ("contactid", "contactid"),
///     ("requestid", "requestid"),
/// ])
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resolver {
    /// Database ID (None if not yet persisted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Unique name for this resolver within the config
    pub name: String,
    /// Target entity to search for matches (logical name)
    pub source_entity: String,
    /// Fields to match against (supports single or compound keys)
    pub match_fields: Vec<MatchField>,
    /// What to do when no match is found
    #[serde(default)]
    pub fallback: ResolverFallback,
}

impl Resolver {
    /// Create a new single-field resolver (backwards compatible)
    ///
    /// For single-field resolution, source and target field names are assumed to be the same.
    pub fn new(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_field: impl Into<String>,
    ) -> Self {
        let field = match_field.into();
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_fields: vec![MatchField::simple(field)],
            fallback: ResolverFallback::default(),
        }
    }

    /// Create a new single-field resolver with custom fallback
    pub fn with_fallback(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_field: impl Into<String>,
        fallback: ResolverFallback,
    ) -> Self {
        let field = match_field.into();
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_fields: vec![MatchField::simple(field)],
            fallback,
        }
    }

    /// Create a compound key resolver with multiple match fields
    ///
    /// Each tuple is (source_field, target_field). For simple cases where
    /// source and target field names match, you can use the same value for both.
    ///
    /// # Example
    /// ```ignore
    /// // Resolve capacity by both contact and request IDs
    /// Resolver::compound("capacity_by_pair", "capacity", vec![
    ///     ("contactid", "contactid"),
    ///     ("requestid", "requestid"),
    /// ])
    /// ```
    pub fn compound(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_fields: Vec<(impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_fields: match_fields
                .into_iter()
                .map(|(source, target)| {
                    MatchField::new(FieldPath::simple(source.into()), target)
                })
                .collect(),
            fallback: ResolverFallback::default(),
        }
    }

    /// Create a compound key resolver with custom fallback
    pub fn compound_with_fallback(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_fields: Vec<(impl Into<String>, impl Into<String>)>,
        fallback: ResolverFallback,
    ) -> Self {
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_fields: match_fields
                .into_iter()
                .map(|(source, target)| {
                    MatchField::new(FieldPath::simple(source.into()), target)
                })
                .collect(),
            fallback,
        }
    }

    /// Create a resolver with full control over match fields
    pub fn with_match_fields(
        name: impl Into<String>,
        source_entity: impl Into<String>,
        match_fields: Vec<MatchField>,
        fallback: ResolverFallback,
    ) -> Self {
        Resolver {
            id: None,
            name: name.into(),
            source_entity: source_entity.into(),
            match_fields,
            fallback,
        }
    }

    /// Check if this is a compound key resolver (more than one match field)
    pub fn is_compound(&self) -> bool {
        self.match_fields.len() > 1
    }

    /// Get all target field names that need to be fetched for this resolver
    pub fn target_fields(&self) -> Vec<&str> {
        self.match_fields.iter().map(|mf| mf.target_field.as_str()).collect()
    }
}

/// Fallback behavior for resolver when no match is found
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverFallback {
    /// Mark the record as an error (won't be transferred)
    #[default]
    Error,
    /// Use null for the lookup field
    Null,
    /// Use a static GUID as the fallback value
    #[serde(rename = "default")]
    Default(uuid::Uuid),
}

impl ResolverFallback {
    /// Get display label for UI
    pub fn label(&self) -> String {
        match self {
            ResolverFallback::Error => "Error".to_string(),
            ResolverFallback::Null => "Null".to_string(),
            ResolverFallback::Default(guid) => format!("Default({})", guid),
        }
    }

    /// Cycle to the next fallback option (cycles through Error -> Null -> Error)
    /// Note: Default requires explicit setting with a GUID, so it's not part of the cycle
    pub fn cycle(&self) -> Self {
        match self {
            ResolverFallback::Error => ResolverFallback::Null,
            ResolverFallback::Null => ResolverFallback::Error,
            ResolverFallback::Default(_) => ResolverFallback::Error,
        }
    }

    /// Check if this is the Default variant
    pub fn is_default(&self) -> bool {
        matches!(self, ResolverFallback::Default(_))
    }

    /// Get the default GUID if this is a Default variant
    pub fn default_guid(&self) -> Option<uuid::Uuid> {
        match self {
            ResolverFallback::Default(guid) => Some(*guid),
            _ => None,
        }
    }
}

use std::collections::HashMap;
use uuid::Uuid;

use super::value::Value;

/// Unit separator character for compound key components
const COMPOUND_KEY_SEPARATOR: char = '\x1F';

/// Runtime context for resolver lookups
///
/// Built from target entity data, provides fast lookups to resolve
/// source values to target record GUIDs. Supports both single-field
/// and compound key matching.
#[derive(Debug, Default)]
pub struct ResolverContext {
    /// Lookup tables: resolver_name -> (composite_key -> guid)
    /// For single-field resolvers, the key is just the normalized value.
    /// For compound resolvers, the key is "field1=value1\x1Ffield2=value2" (sorted by field name).
    tables: HashMap<String, HashMap<String, Uuid>>,
    /// Fallback behavior for each resolver
    fallbacks: HashMap<String, ResolverFallback>,
    /// Match field configurations for each resolver (needed for resolution)
    match_fields: HashMap<String, Vec<MatchField>>,
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

        log::info!(
            "Building ResolverContext: {} resolvers, target_data keys: {:?}, primary_keys: {:?}",
            resolvers.len(),
            target_data.keys().collect::<Vec<_>>(),
            primary_keys
        );

        for resolver in resolvers {
            // Sort match fields by target_field for order-independent composite keys
            let mut sorted_match_fields = resolver.match_fields.clone();
            sorted_match_fields.sort_by(|a, b| a.target_field.cmp(&b.target_field));

            let field_names: Vec<_> = sorted_match_fields.iter().map(|mf| mf.target_field.as_str()).collect();
            log::info!(
                "Processing resolver '{}': source_entity='{}', match_fields={:?}",
                resolver.name,
                resolver.source_entity,
                field_names
            );

            let Some(records) = target_data.get(&resolver.source_entity) else {
                log::warn!(
                    "Resolver '{}' references entity '{}' which has no data",
                    resolver.name,
                    resolver.source_entity
                );
                continue;
            };

            log::info!(
                "Resolver '{}': found {} records for entity '{}'",
                resolver.name,
                records.len(),
                resolver.source_entity
            );

            // Log sample record keys to debug field name issues
            if let Some(first_record) = records.first() {
                if let Some(obj) = first_record.as_object() {
                    let keys: Vec<_> = obj.keys().collect();
                    log::info!(
                        "Resolver '{}': sample record has {} keys: {:?}",
                        resolver.name,
                        keys.len(),
                        keys.iter().take(10).collect::<Vec<_>>()
                    );
                    // Check if all match fields exist (try direct name and _fieldname_value format)
                    for mf in &sorted_match_fields {
                        let lookup_key = format!("_{}_value", mf.target_field);
                        if !obj.contains_key(&mf.target_field) && !obj.contains_key(&lookup_key) {
                            log::warn!(
                                "Resolver '{}': target_field '{}' NOT FOUND in record! Available: {:?}",
                                resolver.name,
                                mf.target_field,
                                keys
                            );
                        }
                    }
                }
            }

            let Some(pk_field) = primary_keys.get(&resolver.source_entity) else {
                log::warn!(
                    "Resolver '{}' references entity '{}' which has no primary key",
                    resolver.name,
                    resolver.source_entity
                );
                continue;
            };

            let mut table: HashMap<String, Uuid> = HashMap::new();
            let mut duplicate_count = 0usize;

            for record in records {
                // Get the primary key value
                let Some(pk_value) = record.get(pk_field).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Ok(guid) = Uuid::parse_str(pk_value) else {
                    continue;
                };

                // Build composite key from all match fields (sorted by target_field)
                let composite_key = Self::build_composite_key_from_record(record, &sorted_match_fields);
                if composite_key.is_empty() {
                    continue;
                }

                // First match wins - skip if already have a value for this key
                if table.contains_key(&composite_key) {
                    duplicate_count += 1;
                } else {
                    table.insert(composite_key, guid);
                }
            }

            if duplicate_count > 0 {
                log::warn!(
                    "Resolver '{}' has {} duplicate composite keys (using first match)",
                    resolver.name,
                    duplicate_count,
                );
            }

            log::info!(
                "Resolver '{}' built lookup table with {} unique entries",
                resolver.name,
                table.len()
            );

            ctx.tables.insert(resolver.name.clone(), table);
            ctx.fallbacks.insert(resolver.name.clone(), resolver.fallback.clone());
            ctx.match_fields.insert(resolver.name.clone(), sorted_match_fields);
        }

        ctx
    }

    /// Build a composite key from a target record using the sorted match fields
    fn build_composite_key_from_record(record: &serde_json::Value, match_fields: &[MatchField]) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(match_fields.len());

        for mf in match_fields {
            // Try direct field name first, then _fieldname_value format (for lookup fields)
            let value = record
                .get(&mf.target_field)
                .or_else(|| record.get(&format!("_{}_value", mf.target_field)));

            let Some(value) = value else {
                return String::new(); // Missing field, skip this record
            };
            let normalized = Self::normalize_value(value);
            if normalized.is_empty() {
                return String::new(); // Empty value, skip this record
            }
            parts.push(format!("{}={}", mf.target_field, normalized));
        }

        parts.join(&COMPOUND_KEY_SEPARATOR.to_string())
    }

    /// Build a composite key from field-value pairs for lookup
    ///
    /// The pairs are sorted by field name to ensure order-independent matching.
    pub fn build_composite_key(pairs: &[(&str, &serde_json::Value)]) -> String {
        // Sort by field name for order-independent matching
        let mut sorted_pairs: Vec<_> = pairs.iter().collect();
        sorted_pairs.sort_by(|a, b| a.0.cmp(b.0));

        let mut parts: Vec<String> = Vec::with_capacity(sorted_pairs.len());
        for (field, value) in sorted_pairs {
            let normalized = Self::normalize_value(value);
            if normalized.is_empty() {
                return String::new(); // Empty value means no match
            }
            parts.push(format!("{}={}", field, normalized));
        }

        parts.join(&COMPOUND_KEY_SEPARATOR.to_string())
    }

    /// Resolve a single value using a specific resolver (for single-field resolvers)
    ///
    /// # Arguments
    /// * `resolver_name` - The name of the resolver to use
    /// * `value` - The value to look up
    ///
    /// # Returns
    /// The resolution result (Found or NotFound)
    #[deprecated(note = "Use resolve_composite for compound key support")]
    pub fn resolve(&self, resolver_name: &str, value: &serde_json::Value) -> ResolveResult {
        // For backwards compatibility with single-field resolvers
        let Some(match_fields) = self.match_fields.get(resolver_name) else {
            return ResolveResult::NotFound;
        };

        if match_fields.len() != 1 {
            log::warn!(
                "resolve() called on compound resolver '{}' with {} fields - use resolve_composite instead",
                resolver_name,
                match_fields.len()
            );
            return ResolveResult::NotFound;
        }

        // Build composite key for single field
        let field_name = &match_fields[0].target_field;
        let pairs = [(field_name.as_str(), value)];
        self.resolve_composite(resolver_name, &pairs)
    }

    /// Resolve using multiple field-value pairs (for compound key resolvers)
    ///
    /// # Arguments
    /// * `resolver_name` - The name of the resolver to use
    /// * `pairs` - Field name and value pairs to match
    ///
    /// # Returns
    /// The resolution result (Found or NotFound)
    pub fn resolve_composite(&self, resolver_name: &str, pairs: &[(&str, &serde_json::Value)]) -> ResolveResult {
        let composite_key = Self::build_composite_key(pairs);
        if composite_key.is_empty() {
            log::debug!(
                "Resolver '{}': empty composite key from pairs: {:?}",
                resolver_name,
                pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>()
            );
            return ResolveResult::NotFound;
        }

        // Look up in the table
        if let Some(table) = self.tables.get(resolver_name) {
            if let Some(guid) = table.get(&composite_key) {
                log::trace!(
                    "Resolver '{}': FOUND key '{}' -> {}",
                    resolver_name,
                    composite_key.replace('\x1F', " | "),
                    guid
                );
                return ResolveResult::Found(*guid);
            }

            // Log first few misses with sample of available keys
            static MISS_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let count = MISS_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count < 5 {
                let sample_keys: Vec<_> = table.keys().take(3).map(|k| k.replace('\x1F', " | ")).collect();
                log::warn!(
                    "Resolver '{}': NOT FOUND key '{}'. Table has {} entries. Sample keys: {:?}",
                    resolver_name,
                    composite_key.replace('\x1F', " | "),
                    table.len(),
                    sample_keys
                );
            }
        } else {
            log::warn!("Resolver '{}': table not found in context", resolver_name);
        }

        ResolveResult::NotFound
    }

    /// Check if a resolver exists in this context
    pub fn has_resolver(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    /// Get the match fields for a resolver
    pub fn get_match_fields(&self, resolver_name: &str) -> Option<&[MatchField]> {
        self.match_fields.get(resolver_name).map(|v| v.as_slice())
    }

    /// Resolve using field-value pairs and return the result as a Value, applying fallback behavior
    ///
    /// This method is used by the transform engine to resolve Copy transforms
    /// that have a resolver specified.
    ///
    /// # Arguments
    /// * `resolver_name` - The name of the resolver to use
    /// * `pairs` - Field name and value pairs to match
    ///
    /// # Returns
    /// * `Ok(Value::Guid(uuid))` - Successfully resolved to a GUID
    /// * `Ok(Value::Null)` - No match found and fallback is Null
    /// * `Err(String)` - No match/duplicate found and fallback is Error
    pub fn resolve_composite_to_value(
        &self,
        resolver_name: &str,
        pairs: &[(&str, &serde_json::Value)],
    ) -> Result<Value, String> {
        let result = self.resolve_composite(resolver_name, pairs);
        let fallback = self
            .fallbacks
            .get(resolver_name)
            .cloned()
            .unwrap_or(ResolverFallback::Error);

        match result {
            ResolveResult::Found(guid) => Ok(Value::Guid(guid)),
            ResolveResult::NotFound => match fallback {
                ResolverFallback::Error => {
                    let display_values: Vec<_> = pairs
                        .iter()
                        .map(|(field, value)| format!("{}={}", field, Self::normalize_value(value)))
                        .collect();
                    Err(format!(
                        "Resolver '{}': no match found for [{}]",
                        resolver_name,
                        display_values.join(", ")
                    ))
                }
                ResolverFallback::Null => Ok(Value::Null),
                ResolverFallback::Default(guid) => Ok(Value::Guid(guid)),
            },
            ResolveResult::Duplicate => unreachable!("Duplicates are handled by first-match-wins"),
        }
    }

    /// Resolve a single value and return the result as a Value (backwards compatible)
    ///
    /// For single-field resolvers only. Use resolve_composite_to_value for compound keys.
    pub fn resolve_to_value(
        &self,
        resolver_name: &str,
        value: &serde_json::Value,
    ) -> Result<Value, String> {
        // For backwards compatibility with single-field resolvers
        let Some(match_fields) = self.match_fields.get(resolver_name) else {
            return Err(format!("Resolver '{}' not found", resolver_name));
        };

        if match_fields.len() != 1 {
            return Err(format!(
                "resolve_to_value() called on compound resolver '{}' with {} fields - use resolve_composite_to_value instead",
                resolver_name,
                match_fields.len()
            ));
        }

        // Build composite key for single field
        let field_name = &match_fields[0].target_field;
        let pairs = [(field_name.as_str(), value)];
        self.resolve_composite_to_value(resolver_name, &pairs)
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
        assert_eq!(resolver.match_fields.len(), 1);
        assert_eq!(resolver.match_fields[0].target_field, "emailaddress1");
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
    fn test_resolver_compound() {
        let resolver = Resolver::compound(
            "capacity_by_pair",
            "capacity",
            vec![("contactid", "contactid"), ("requestid", "requestid")],
        );
        assert_eq!(resolver.name, "capacity_by_pair");
        assert_eq!(resolver.source_entity, "capacity");
        assert_eq!(resolver.match_fields.len(), 2);
        assert!(resolver.is_compound());
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
    #[allow(deprecated)]
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
    #[allow(deprecated)]
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

        // Duplicates use first-match-wins strategy
        let result = ctx.resolve("contact_by_email", &json!("duplicate@example.com"));
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );

        // Unique should still work
        let result = ctx.resolve("contact_by_email", &json!("unique@example.com"));
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap())
        );
    }

    #[test]
    #[allow(deprecated)]
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
    #[allow(deprecated)]
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

    #[test]
    fn test_resolver_compound_key_build_and_resolve() {
        use serde_json::json;

        // Create a compound key resolver for capacity (junction entity)
        let resolvers = vec![Resolver::compound(
            "capacity_by_pair",
            "capacity",
            vec![("contactid", "contactid"), ("requestid", "requestid")],
        )];

        let mut target_data = HashMap::new();
        target_data.insert(
            "capacity".to_string(),
            vec![
                json!({
                    "capacityid": "11111111-1111-1111-1111-111111111111",
                    "contactid": "aaaa1111-1111-1111-1111-111111111111",
                    "requestid": "bbbb1111-1111-1111-1111-111111111111"
                }),
                json!({
                    "capacityid": "22222222-2222-2222-2222-222222222222",
                    "contactid": "aaaa2222-2222-2222-2222-222222222222",
                    "requestid": "bbbb2222-2222-2222-2222-222222222222"
                }),
                json!({
                    "capacityid": "33333333-3333-3333-3333-333333333333",
                    "contactid": "aaaa1111-1111-1111-1111-111111111111",  // Same contact as first
                    "requestid": "bbbb3333-3333-3333-3333-333333333333"   // Different request
                }),
            ],
        );

        let mut primary_keys = HashMap::new();
        primary_keys.insert("capacity".to_string(), "capacityid".to_string());

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Test successful compound lookup
        let pairs = [
            ("contactid", &json!("aaaa1111-1111-1111-1111-111111111111") as &serde_json::Value),
            ("requestid", &json!("bbbb1111-1111-1111-1111-111111111111")),
        ];
        let result = ctx.resolve_composite("capacity_by_pair", &pairs);
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );

        // Test with reversed order (should still work due to order-independent matching)
        let pairs_reversed = [
            ("requestid", &json!("bbbb1111-1111-1111-1111-111111111111") as &serde_json::Value),
            ("contactid", &json!("aaaa1111-1111-1111-1111-111111111111")),
        ];
        let result = ctx.resolve_composite("capacity_by_pair", &pairs_reversed);
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );

        // Test partial match (only one field matches) - should not find
        let partial_pairs = [
            ("contactid", &json!("aaaa1111-1111-1111-1111-111111111111") as &serde_json::Value),
            ("requestid", &json!("wrong-request-id")),
        ];
        let result = ctx.resolve_composite("capacity_by_pair", &partial_pairs);
        assert_eq!(result, ResolveResult::NotFound);

        // Test with second record
        let pairs2 = [
            ("contactid", &json!("aaaa2222-2222-2222-2222-222222222222") as &serde_json::Value),
            ("requestid", &json!("bbbb2222-2222-2222-2222-222222222222")),
        ];
        let result = ctx.resolve_composite("capacity_by_pair", &pairs2);
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
        );
    }

    #[test]
    fn test_resolver_compound_key_case_insensitive() {
        use serde_json::json;

        let resolvers = vec![Resolver::compound(
            "capacity_by_pair",
            "capacity",
            vec![("contactid", "contactid"), ("requestid", "requestid")],
        )];

        let mut target_data = HashMap::new();
        target_data.insert(
            "capacity".to_string(),
            vec![json!({
                "capacityid": "11111111-1111-1111-1111-111111111111",
                "contactid": "AAAA1111-1111-1111-1111-111111111111",  // Uppercase
                "requestid": "bbbb1111-1111-1111-1111-111111111111"
            })],
        );

        let mut primary_keys = HashMap::new();
        primary_keys.insert("capacity".to_string(), "capacityid".to_string());

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Test case-insensitive lookup (lowercase query should match uppercase data)
        let pairs = [
            ("contactid", &json!("aaaa1111-1111-1111-1111-111111111111") as &serde_json::Value),
            ("requestid", &json!("BBBB1111-1111-1111-1111-111111111111")),
        ];
        let result = ctx.resolve_composite("capacity_by_pair", &pairs);
        assert_eq!(
            result,
            ResolveResult::Found(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        );
    }

    #[test]
    fn test_resolver_compound_to_value() {
        use serde_json::json;

        let resolvers = vec![Resolver::compound_with_fallback(
            "capacity_by_pair",
            "capacity",
            vec![("contactid", "contactid"), ("requestid", "requestid")],
            ResolverFallback::Null,
        )];

        let mut target_data = HashMap::new();
        target_data.insert(
            "capacity".to_string(),
            vec![json!({
                "capacityid": "11111111-1111-1111-1111-111111111111",
                "contactid": "aaaa1111-1111-1111-1111-111111111111",
                "requestid": "bbbb1111-1111-1111-1111-111111111111"
            })],
        );

        let mut primary_keys = HashMap::new();
        primary_keys.insert("capacity".to_string(), "capacityid".to_string());

        let ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        // Test successful resolution
        let pairs = [
            ("contactid", &json!("aaaa1111-1111-1111-1111-111111111111") as &serde_json::Value),
            ("requestid", &json!("bbbb1111-1111-1111-1111-111111111111")),
        ];
        let result = ctx.resolve_composite_to_value("capacity_by_pair", &pairs);
        assert_eq!(
            result,
            Ok(Value::Guid(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()))
        );

        // Test fallback to null
        let not_found_pairs = [
            ("contactid", &json!("not-found") as &serde_json::Value),
            ("requestid", &json!("not-found")),
        ];
        let result = ctx.resolve_composite_to_value("capacity_by_pair", &not_found_pairs);
        assert_eq!(result, Ok(Value::Null));
    }

    #[test]
    fn test_composite_key_format() {
        use serde_json::json;

        // Test the composite key building
        let pairs = [
            ("b_field", &json!("value_b") as &serde_json::Value),
            ("a_field", &json!("value_a")),
        ];
        let key = ResolverContext::build_composite_key(&pairs);

        // Should be sorted alphabetically by field name
        assert!(key.contains("a_field=value_a"));
        assert!(key.contains("b_field=value_b"));
        // a_field should come before b_field
        let a_pos = key.find("a_field").unwrap();
        let b_pos = key.find("b_field").unwrap();
        assert!(a_pos < b_pos);
    }
}
