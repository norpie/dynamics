//! Transform engine - orchestrates applying transforms to source records

use std::collections::HashMap;
use uuid::Uuid;

use crate::transfer::{
    EntityMapping, FieldMapping, RecordAction, ResolvedEntity, ResolvedRecord,
    ResolvedTransfer, TransferConfig, Value,
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
    /// Primary key field name for the current entity
    pub primary_key_field: String,
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

        for entity_mapping in config.entity_mappings_by_priority() {
            let source_records = source_data
                .get(&entity_mapping.source_entity)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let target_records = target_data
                .get(&entity_mapping.target_entity)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let pk_field = primary_keys
                .get(&entity_mapping.source_entity)
                .cloned()
                .unwrap_or_else(|| format!("{}id", entity_mapping.source_entity));

            let ctx = TransformContext {
                primary_key_field: pk_field,
            };

            let resolved_entity =
                Self::transform_entity(entity_mapping, source_records, target_records, &ctx);
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
    ) -> ResolvedEntity {
        let mut resolved = ResolvedEntity::new(
            &mapping.target_entity,
            mapping.priority,
            &ctx.primary_key_field,
        );

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
                r.get(&ctx.primary_key_field)
                    .and_then(|v| v.as_str())
                    .map(|id| (id.to_string(), r))
            })
            .collect();

        for record in source_records {
            let resolved_record = Self::transform_record(
                record,
                &mapping.field_mappings,
                &target_index,
                &field_names,
                ctx,
            );
            resolved.add_record(resolved_record);
        }

        resolved
    }

    /// Transform a single record and compare against target
    pub fn transform_record(
        source: &serde_json::Value,
        field_mappings: &[FieldMapping],
        target_index: &HashMap<String, &serde_json::Value>,
        field_names: &[String],
        ctx: &TransformContext,
    ) -> ResolvedRecord {
        // Extract source ID
        let source_id_str = source
            .get(&ctx.primary_key_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let source_id = Uuid::parse_str(source_id_str).unwrap_or_else(|_| Uuid::new_v4());

        let mut fields = HashMap::new();
        let mut errors = Vec::new();

        for field_mapping in field_mappings {
            match apply_transform(&field_mapping.transform, source) {
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
        if let Some(target) = target_index.get(source_id_str) {
            if Self::fields_match(&fields, target, field_names) {
                return ResolvedRecord::nochange(source_id, fields);
            }
        }

        ResolvedRecord::upsert(source_id, fields)
    }

    /// Compare resolved fields against target record
    fn fields_match(
        resolved: &HashMap<String, Value>,
        target: &serde_json::Value,
        field_names: &[String],
    ) -> bool {
        for field_name in field_names {
            let resolved_value = resolved.get(field_name);
            let target_value = target.get(field_name);

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
    use crate::transfer::{FieldPath, Transform};
    use serde_json::json;

    fn make_ctx() -> TransformContext {
        TransformContext {
            primary_key_field: "accountid".to_string(),
        }
    }

    #[test]
    fn test_transform_record_upsert_when_no_target() {
        let source = json!({
            "accountid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Contoso",
            "revenue": 1000000
        });

        let mappings = vec![
            FieldMapping::new("name", Transform::Copy {
                source_path: FieldPath::simple("name"),
            }),
            FieldMapping::new("was_migrated", Transform::Constant {
                value: Value::Bool(true),
            }),
        ];

        let target_index = HashMap::new(); // No target records
        let field_names = vec!["name".to_string(), "was_migrated".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx()
        );

        assert!(result.is_upsert());
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
            }),
        ];

        let mut target_index = HashMap::new();
        target_index.insert("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(), &target);
        let field_names = vec!["name".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx()
        );

        assert!(result.is_nochange());
    }

    #[test]
    fn test_transform_record_upsert_when_target_differs() {
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
            }),
        ];

        let mut target_index = HashMap::new();
        target_index.insert("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(), &target);
        let field_names = vec!["name".to_string()];

        let result = TransformEngine::transform_record(
            &source, &mappings, &target_index, &field_names, &make_ctx()
        );

        assert!(result.is_upsert());
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
            &source, &mappings, &target_index, &field_names, &make_ctx()
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
        // Target doesn't have Fabrikam -> Upsert
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
            field_mappings: vec![
                FieldMapping::new("name", Transform::Copy {
                    source_path: FieldPath::simple("name"),
                }),
            ],
        };

        let result = TransformEngine::transform_entity(
            &mapping, &source_records, &target_records, &make_ctx()
        );

        assert_eq!(result.entity_name, "account");
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.nochange_count(), 1);
        assert_eq!(result.upsert_count(), 1);
    }

    #[test]
    fn test_transform_all() {
        let config = TransferConfig {
            id: None,
            name: "test-migration".to_string(),
            source_env: "dev".to_string(),
            target_env: "prod".to_string(),
            entity_mappings: vec![
                EntityMapping {
                    id: None,
                    source_entity: "account".to_string(),
                    target_entity: "account".to_string(),
                    priority: 1,
                    field_mappings: vec![
                        FieldMapping::new("name", Transform::Copy {
                            source_path: FieldPath::simple("name"),
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
        assert_eq!(result.upsert_count(), 1);
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
}
