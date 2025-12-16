//! Transform engine - orchestrates applying transforms to source records

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::transfer::{
    EntityMapping, FieldMapping, RecordAction, ResolvedEntity, ResolvedRecord,
    ResolvedTransfer, ResolverContext, TransferConfig, Value,
};

use super::apply::apply_transform;

/// Error from transform operations
#[derive(Debug, Clone)]
pub struct TransformError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Context for transform operations (future: could hold caches, etc.)
#[derive(Debug, Default)]
pub struct TransformContext {
    /// Primary key field name for source records
    pub source_pk_field: String,
    /// Primary key field name for target records
    pub target_pk_field: String,
}

/// Transform engine for applying mappings to source records
pub struct TransformEngine;

impl TransformEngine {
    /// Transform all entities in a config given pre-fetched source and target data
    ///
    /// `source_data` is a map of entity name -> records
    /// `target_data` is a map of entity name -> records (for comparison)
    pub fn transform_all(
        config: &TransferConfig,
        source_data: &HashMap<String, Vec<serde_json::Value>>,
        target_data: &HashMap<String, Vec<serde_json::Value>>,
        primary_keys: &HashMap<String, String>,
    ) -> ResolvedTransfer {
        let mut resolved = ResolvedTransfer::new(
            &config.name,
            &config.source_env,
            &config.target_env,
        );

        // Build resolver context from config's resolvers and target data
        let resolver_ctx = ResolverContext::build(&config.resolvers, target_data, primary_keys);

        for entity_mapping in config.entity_mappings_by_priority() {
            let source_records = source_data
                .get(&entity_mapping.source_entity)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let target_records = target_data
                .get(&entity_mapping.target_entity)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let source_pk = primary_keys
                .get(&entity_mapping.source_entity)
                .cloned()
                .unwrap_or_else(|| format!("{}id", entity_mapping.source_entity));

            let target_pk = primary_keys
                .get(&entity_mapping.target_entity)
                .cloned()
                .unwrap_or_else(|| format!("{}id", entity_mapping.target_entity));

            let ctx = TransformContext {
                source_pk_field: source_pk,
                target_pk_field: target_pk,
            };

            let resolved_entity =
                Self::transform_entity(entity_mapping, source_records, target_records, &ctx, &resolver_ctx);
            resolved.add_entity(resolved_entity);
        }

        resolved
    }

    /// Transform records for a single entity mapping
    pub fn transform_entity(
        mapping: &EntityMapping,
        source_records: &[serde_json::Value],
        target_records: &[serde_json::Value],
        ctx: &TransformContext,
        resolver_ctx: &ResolverContext,
    ) -> ResolvedEntity {
        let mut resolved = ResolvedEntity::new(
            &mapping.target_entity,
            mapping.priority,
            &ctx.target_pk_field,
        );
        resolved.set_orphan_handling(mapping.orphan_handling);

        // Collect field names from mappings
        let field_names: Vec<String> = mapping
            .field_mappings
            .iter()
            .map(|f| f.target_field.clone())
            .collect();
        resolved.set_field_names(field_names.clone());

        // Index target records by primary key for fast lookup
        let target_index: HashMap<String, &serde_json::Value> = target_records
            .iter()
            .filter_map(|r| {
                r.get(&ctx.target_pk_field)
                    .and_then(|v| v.as_str())
                    .map(|id| (id.to_string(), r))
            })
            .collect();

        log::debug!(
            "Transform entity '{}': {} source records, {} target records, {} indexed by target pk '{}'",
            mapping.target_entity,
            source_records.len(),
            target_records.len(),
            target_index.len(),
            ctx.target_pk_field
        );

        // Debug: show sample IDs if there's a mismatch
        if !source_records.is_empty() && !target_records.is_empty() && target_index.is_empty() {
            if let Some(first_target) = target_records.first() {
                log::warn!(
                    "Target index is empty! Sample target record keys: {:?}",
                    first_target.as_object().map(|o| o.keys().collect::<Vec<_>>())
                );
            }
        }

        // Build set of source IDs for target-only detection
        let source_ids: HashSet<String> = source_records
            .iter()
            .filter_map(|r| {
                r.get(&ctx.source_pk_field)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        for record in source_records {
            let resolved_record = Self::transform_record(
                record,
                &mapping.field_mappings,
                &target_index,
                &field_names,
                ctx,
                resolver_ctx,
            );
            resolved.add_record(resolved_record);
        }

        // Find target-only records (exist in target but not in source)
        for target in target_records {
            let target_id_str = target
                .get(&ctx.target_pk_field)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !source_ids.contains(target_id_str) {
                // This record exists only in target
                let target_id = Uuid::parse_str(target_id_str).unwrap_or_else(|_| Uuid::new_v4());
                let fields = Self::extract_fields_from_target(target, &field_names);
                resolved.add_record(ResolvedRecord::target_only(target_id, fields));
            }
        }

        resolved
    }

    /// Extract field values from a target record for display
    fn extract_fields_from_target(
        target: &serde_json::Value,
        field_names: &[String],
    ) -> HashMap<String, Value> {
        let mut fields = HashMap::new();
        for field_name in field_names {
            // Try direct field name first, then OData lookup format (_fieldname_value)
            let json_value = target
                .get(field_name)
                .or_else(|| target.get(&format!("_{}_value", field_name)));
            if let Some(json_value) = json_value {
                let value = Value::from_json(json_value);
                fields.insert(field_name.clone(), value);
            }
        }
        fields
    }

    /// Transform a single record and compare against target
    pub fn transform_record(
        source: &serde_json::Value,
        field_mappings: &[FieldMapping],
        target_index: &HashMap<String, &serde_json::Value>,
        field_names: &[String],
        ctx: &TransformContext,
        resolver_ctx: &ResolverContext,
    ) -> ResolvedRecord {
        // Extract source ID using source pk field
        let source_id_str = source
            .get(&ctx.source_pk_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let source_id = Uuid::parse_str(source_id_str).unwrap_or_else(|_| Uuid::new_v4());

        let mut fields = HashMap::new();
        let mut errors = Vec::new();

        for field_mapping in field_mappings {
            match apply_transform(&field_mapping.transform, source, Some(resolver_ctx)) {
                Ok(value) => {
                    fields.insert(field_mapping.target_field.clone(), value);
                }
                Err(msg) => {
                    errors.push(TransformError {
                        field: field_mapping.target_field.clone(),
                        message: msg,
                    });
                }
            }
        }

        if !errors.is_empty() {
            let error_msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return ResolvedRecord::error_with_fields(source_id, fields, error_msg);
        }

        // Check if target record exists and compare
        let found_in_target = target_index.get(source_id_str);

        // Log first few lookups to debug ID matching issues
        static LOGGED_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = LOGGED_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count < 5 {
            log::debug!(
                "Lookup #{}: source_id='{}', found_in_target={}, target_index_size={}",
                count,
                source_id_str,
                found_in_target.is_some(),
                target_index.len()
            );
            if !target_index.is_empty() && found_in_target.is_none() {
                // Show a sample target key for comparison
                if let Some(sample_key) = target_index.keys().next() {
                    log::debug!("  Sample target key: '{}'", sample_key);
                }
            }
        }

        if let Some(target) = found_in_target {
            // Target exists - check if fields match
            if Self::fields_match(&fields, target, field_names) {
                return ResolvedRecord::nochange(source_id, fields);
            } else {
                // Target exists but fields differ → Update
                return ResolvedRecord::update(source_id, fields);
            }
        }

        // Target doesn't exist → Create
        ResolvedRecord::create(source_id, fields)
    }

    /// Compare resolved fields against target record
    fn fields_match(
        resolved: &HashMap<String, Value>,
        target: &serde_json::Value,
        field_names: &[String],
    ) -> bool {
        for field_name in field_names {
            let resolved_value = resolved.get(field_name);
            // Try direct field name first, then OData lookup format (_fieldname_value)
            let target_value = target
                .get(field_name)
                .or_else(|| target.get(&format!("_{}_value", field_name)));

            match (resolved_value, target_value) {
                // Both null/missing -> match
                (None, None) => continue,
                (Some(Value::Null), None) => continue,
                (None, Some(serde_json::Value::Null)) => continue,
                (Some(Value::Null), Some(serde_json::Value::Null)) => continue,

                // One exists, other doesn't -> no match
                (None, Some(_)) => return false,
                (Some(_), None) => return false,

                // Both exist -> compare
                (Some(resolved_val), Some(target_val)) => {
                    if !Self::values_equal(resolved_val, target_val) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Compare a resolved Value against a JSON value
    fn values_equal(resolved: &Value, target: &serde_json::Value) -> bool {
        match (resolved, target) {
            (Value::Null, serde_json::Value::Null) => true,
            (Value::String(a), serde_json::Value::String(b)) => a == b,
            (Value::Int(a), serde_json::Value::Number(b)) => {
                b.as_i64().map(|b| *a == b).unwrap_or(false)
            }
            (Value::Float(a), serde_json::Value::Number(b)) => {
                b.as_f64().map(|b| (*a - b).abs() < f64::EPSILON).unwrap_or(false)
            }
            (Value::Bool(a), serde_json::Value::Bool(b)) => a == b,
            (Value::Guid(a), serde_json::Value::String(b)) => {
                Uuid::parse_str(b).map(|b| *a == b).unwrap_or(false)
            }
            (Value::OptionSet(a), serde_json::Value::Number(b)) => {
                b.as_i64().map(|b| *a as i64 == b).unwrap_or(false)
            }
            (Value::DateTime(a), serde_json::Value::String(b)) => {
                chrono::DateTime::parse_from_rfc3339(b)
                    .map(|b| *a == b.with_timezone(&chrono::Utc))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{FieldPath, OrphanHandling, Transform};
    use serde_json::json;

    fn make_ctx() -> TransformContext {
        TransformContext {
            source_pk_field: "accountid".to_string(),
            target_pk_field: "accountid".to_string(),
        }
    }

    fn empty_resolver_ctx() -> ResolverContext {
        ResolverContext::default()
    }

    #[test]
    fn test_transform_record_create_when_no_target() {
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Contoso",
            "revenue": 1000000
        });

        let mappings = vec![
            FieldMapping::new("name", Transform::Copy {
                source_path: FieldPath::simple("name"),
                resolver: None,
            }),
            FieldMapping::new("was_migrated", Transform::Constant {
                value: Value::Bool(true),
            }),
        ];

        let target_index = HashMap::new(); // No target records
        let field_names = vec!["name".to_string(), "was_migrated".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &empty_resolver_ctx()
        );

        assert!(result.is_create());
        assert_eq!(result.get_field("name"), Some(&Value::String("Contoso".into())));
        assert_eq!(result.get_field("was_migrated"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_transform_record_nochange_when_target_matches() {
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Contoso"
        });

        let target = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Contoso"
        });

        let mappings = vec![
            FieldMapping::new("name", Transform::Copy {
                source_path: FieldPath::simple("name"),
                resolver: None,
            }),
        ];

        let mut target_index = HashMap::new();
        target_index.insert("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(), &target);
        let field_names = vec!["name".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &empty_resolver_ctx()
        );

        assert!(result.is_nochange());
    }

    #[test]
    fn test_transform_record_update_when_target_differs() {
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Contoso Updated"
        });

        let target = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Contoso"
        });

        let mappings = vec![
            FieldMapping::new("name", Transform::Copy {
                source_path: FieldPath::simple("name"),
                resolver: None,
            }),
        ];

        let mut target_index = HashMap::new();
        target_index.insert("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(), &target);
        let field_names = vec!["name".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &empty_resolver_ctx()
        );

        assert!(result.is_update());
    }

    #[test]
    fn test_transform_record_with_error() {
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "gendercode": 99
        });

        let mappings = vec![
            FieldMapping::new("gendercode", Transform::ValueMap {
                source_path: FieldPath::simple("gendercode"),
                mappings: vec![
                    (Value::Int(1), Value::Int(100)),
                ],
                fallback: crate::transfer::Fallback::Error,
            }),
        ];

        let target_index = HashMap::new();
        let field_names = vec!["gendercode".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &empty_resolver_ctx()
        );

        assert!(result.is_error());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_transform_entity_with_mixed_results() {
        let source_records = vec![
            json!({
                "accountid": "a1b2c3d4-0000-0000-0000-000000000001",
                "name": "Contoso"
            }),
            json!({
                "accountid": "a1b2c3d4-0000-0000-0000-000000000002",
                "name": "Fabrikam"
            }),
        ];

        // Target has Contoso with same name -> NoChange
        // Target doesn't have Fabrikam -> Create
        let target_records = vec![
            json!({
                "accountid": "a1b2c3d4-0000-0000-0000-000000000001",
                "name": "Contoso"
            }),
        ];

        let mapping = EntityMapping {
            id: None,
            source_entity: "account".to_string(),
            target_entity: "account".to_string(),
            priority: 1,
            orphan_handling: OrphanHandling::default(),
            field_mappings: vec![
                FieldMapping::new("name", Transform::Copy {
                    source_path: FieldPath::simple("name"),
                    resolver: None,
                }),
            ],
        };

        let result = TransformEngine::transform_entity(
            &mapping, &source_records, &target_records, &make_ctx(), &empty_resolver_ctx()
        );

        assert_eq!(result.entity_name, "account");
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.nochange_count(), 1);
        assert_eq!(result.create_count(), 1);
    }

    #[test]
    fn test_transform_all() {
        let config = TransferConfig {
            id: None,
            name: "test-migration".to_string(),
            source_env: "dev".to_string(),
            target_env: "prod".to_string(),
            resolvers: Vec::new(),
            entity_mappings: vec![
                EntityMapping {
                    id: None,
                    source_entity: "account".to_string(),
                    target_entity: "account".to_string(),
                    priority: 1,
                    orphan_handling: OrphanHandling::default(),
                    field_mappings: vec![
                        FieldMapping::new("name", Transform::Copy {
                            source_path: FieldPath::simple("name"),
                            resolver: None,
                        }),
                    ],
                },
            ],
        };

        let mut source_data = HashMap::new();
        source_data.insert("account".to_string(), vec![
            json!({"accountid": "a1b2c3d4-0000-0000-0000-000000000001", "name": "Test"}),
        ]);

        let target_data = HashMap::new(); // Empty target

        let mut primary_keys = HashMap::new();
        primary_keys.insert("account".to_string(), "accountid".to_string());

        let result = TransformEngine::transform_all(&config, &source_data, &target_data, &primary_keys);

        assert_eq!(result.config_name, "test-migration");
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.total_records(), 1);
        assert_eq!(result.create_count(), 1);
    }

    #[test]
    fn test_values_equal() {
        // String
        assert!(TransformEngine::values_equal(
            &Value::String("test".to_string()),
            &json!("test")
        ));

        // Int
        assert!(TransformEngine::values_equal(&Value::Int(42), &json!(42)));

        // Bool
        assert!(TransformEngine::values_equal(&Value::Bool(true), &json!(true)));

        // Guid
        let guid = Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap();
        assert!(TransformEngine::values_equal(
            &Value::Guid(guid),
            &json!("a1b2c3d4-e5f6-7890-abcd-ef1234567890")
        ));

        // Mismatch
        assert!(!TransformEngine::values_equal(
            &Value::String("a".to_string()),
            &json!("b")
        ));
    }

    #[test]
    fn test_nochange_with_odata_lookup_field_format() {
        // Verifies that lookup fields stored in OData format (_fieldname_value)
        // are correctly matched against resolved fields (fieldname)
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "parentaccountid": "11111111-1111-1111-1111-111111111111"
        });

        // Target uses OData format: _parentaccountid_value instead of parentaccountid
        let target = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "_parentaccountid_value": "11111111-1111-1111-1111-111111111111"
        });

        let mappings = vec![
            FieldMapping::new("parentaccountid", Transform::Copy {
                source_path: FieldPath::simple("parentaccountid"),
                resolver: None,
            }),
        ];

        let mut target_index = HashMap::new();
        target_index.insert("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(), &target);
        let field_names = vec!["parentaccountid".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &empty_resolver_ctx()
        );

        // Should be NoChange because the GUID values match (despite different key names)
        assert!(result.is_nochange(), "Expected NoChange but got {:?}", result.action);
    }

    #[test]
    fn test_update_with_odata_lookup_field_different_value() {
        // Verifies that different lookup values are correctly detected as Update
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "parentaccountid": "22222222-2222-2222-2222-222222222222"
        });

        let target = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "_parentaccountid_value": "11111111-1111-1111-1111-111111111111"
        });

        let mappings = vec![
            FieldMapping::new("parentaccountid", Transform::Copy {
                source_path: FieldPath::simple("parentaccountid"),
                resolver: None,
            }),
        ];

        let mut target_index = HashMap::new();
        target_index.insert("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(), &target);
        let field_names = vec!["parentaccountid".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &empty_resolver_ctx()
        );

        // Should be Update because the GUID values differ
        assert!(result.is_update(), "Expected Update but got {:?}", result.action);
    }

    // Resolver integration tests

    #[test]
    fn test_transform_with_resolver_found() {
        use crate::transfer::Resolver;

        // Setup: source record with email, resolver to look up contact by email
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "primary_contact_email": "john@example.com"
        });

        let mappings = vec![
            FieldMapping::new("primarycontactid", Transform::Copy {
                source_path: FieldPath::simple("primary_contact_email"),
                resolver: Some("contact_by_email".to_string()),
            }),
        ];

        // Build resolver context with target contact data
        let resolvers = vec![
            Resolver::new("contact_by_email", "contact", "emailaddress1"),
        ];

        let mut target_data = HashMap::new();
        target_data.insert("contact".to_string(), vec![
            json!({
                "contactid": "11111111-1111-1111-1111-111111111111",
                "emailaddress1": "john@example.com"
            }),
        ]);

        let mut primary_keys = HashMap::new();
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let resolver_ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        let target_index = HashMap::new();
        let field_names = vec!["primarycontactid".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &resolver_ctx
        );

        // Should create with resolved GUID
        assert!(result.is_create());
        let expected_guid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        assert_eq!(result.get_field("primarycontactid"), Some(&Value::Guid(expected_guid)));
    }

    #[test]
    fn test_transform_with_resolver_not_found_error() {
        use crate::transfer::Resolver;

        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "primary_contact_email": "unknown@example.com"
        });

        let mappings = vec![
            FieldMapping::new("primarycontactid", Transform::Copy {
                source_path: FieldPath::simple("primary_contact_email"),
                resolver: Some("contact_by_email".to_string()),
            }),
        ];

        // Resolver with fallback Error (default)
        let resolvers = vec![
            Resolver::new("contact_by_email", "contact", "emailaddress1"),
        ];

        let mut target_data = HashMap::new();
        target_data.insert("contact".to_string(), vec![
            json!({
                "contactid": "11111111-1111-1111-1111-111111111111",
                "emailaddress1": "john@example.com"  // Different email!
            }),
        ]);

        let mut primary_keys = HashMap::new();
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let resolver_ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        let target_index = HashMap::new();
        let field_names = vec!["primarycontactid".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &resolver_ctx
        );

        // Should be marked as error because no match found and fallback is Error
        assert!(result.is_error());
        assert!(result.error.as_ref().unwrap().contains("no match found"));
    }

    #[test]
    fn test_transform_with_resolver_not_found_null() {
        use crate::transfer::{Resolver, ResolverFallback};

        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "primary_contact_email": "unknown@example.com"
        });

        let mappings = vec![
            FieldMapping::new("primarycontactid", Transform::Copy {
                source_path: FieldPath::simple("primary_contact_email"),
                resolver: Some("contact_by_email".to_string()),
            }),
        ];

        // Resolver with fallback Null
        let resolvers = vec![
            Resolver::with_fallback("contact_by_email", "contact", "emailaddress1", ResolverFallback::Null),
        ];

        let mut target_data = HashMap::new();
        target_data.insert("contact".to_string(), vec![
            json!({
                "contactid": "11111111-1111-1111-1111-111111111111",
                "emailaddress1": "john@example.com"
            }),
        ]);

        let mut primary_keys = HashMap::new();
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let resolver_ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        let target_index = HashMap::new();
        let field_names = vec!["primarycontactid".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &resolver_ctx
        );

        // Should create with null value because fallback is Null
        assert!(result.is_create());
        assert_eq!(result.get_field("primarycontactid"), Some(&Value::Null));
    }

    #[test]
    fn test_transform_with_resolver_duplicate() {
        use crate::transfer::Resolver;

        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "primary_contact_email": "duplicate@example.com"
        });

        let mappings = vec![
            FieldMapping::new("primarycontactid", Transform::Copy {
                source_path: FieldPath::simple("primary_contact_email"),
                resolver: Some("contact_by_email".to_string()),
            }),
        ];

        let resolvers = vec![
            Resolver::new("contact_by_email", "contact", "emailaddress1"),
        ];

        // Two contacts with same email - ambiguous!
        let mut target_data = HashMap::new();
        target_data.insert("contact".to_string(), vec![
            json!({
                "contactid": "11111111-1111-1111-1111-111111111111",
                "emailaddress1": "duplicate@example.com"
            }),
            json!({
                "contactid": "22222222-2222-2222-2222-222222222222",
                "emailaddress1": "duplicate@example.com"
            }),
        ]);

        let mut primary_keys = HashMap::new();
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let resolver_ctx = ResolverContext::build(&resolvers, &target_data, &primary_keys);

        let target_index = HashMap::new();
        let field_names = vec!["primarycontactid".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx(), &resolver_ctx
        );

        // Should be marked as error because duplicate values are ambiguous
        assert!(result.is_error());
        assert!(result.error.as_ref().unwrap().contains("multiple matches"));
    }

    #[test]
    fn test_transform_all_with_resolver() {
        use crate::transfer::Resolver;

        let config = TransferConfig {
            id: None,
            name: "test-with-resolver".to_string(),
            source_env: "dev".to_string(),
            target_env: "prod".to_string(),
            resolvers: vec![
                Resolver::new("contact_by_email", "contact", "emailaddress1"),
            ],
            entity_mappings: vec![
                EntityMapping {
                    id: None,
                    source_entity: "account".to_string(),
                    target_entity: "account".to_string(),
                    priority: 1,
                    orphan_handling: OrphanHandling::default(),
                    field_mappings: vec![
                        FieldMapping::new("name", Transform::Copy {
                            source_path: FieldPath::simple("name"),
                            resolver: None,
                        }),
                        FieldMapping::new("primarycontactid", Transform::Copy {
                            source_path: FieldPath::simple("contact_email"),
                            resolver: Some("contact_by_email".to_string()),
                        }),
                    ],
                },
            ],
        };

        let mut source_data = HashMap::new();
        source_data.insert("account".to_string(), vec![
            json!({
                "accountid": "a1b2c3d4-0000-0000-0000-000000000001",
                "name": "Test Account",
                "contact_email": "john@example.com"
            }),
        ]);

        let mut target_data = HashMap::new();
        // Contact data for resolver
        target_data.insert("contact".to_string(), vec![
            json!({
                "contactid": "11111111-1111-1111-1111-111111111111",
                "emailaddress1": "john@example.com"
            }),
        ]);

        let mut primary_keys = HashMap::new();
        primary_keys.insert("account".to_string(), "accountid".to_string());
        primary_keys.insert("contact".to_string(), "contactid".to_string());

        let result = TransformEngine::transform_all(&config, &source_data, &target_data, &primary_keys);

        assert_eq!(result.config_name, "test-with-resolver");
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.create_count(), 1);

        // Verify the resolved GUID
        let account_entity = &result.entities[0];
        let record = &account_entity.records[0];
        let expected_guid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        assert_eq!(record.get_field("primarycontactid"), Some(&Value::Guid(expected_guid)));
    }
}
