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
    /// Transform all entities in a config given pre-fetched source data
    ///
    /// `source_data` is a map of entity name -> records
    pub fn transform_all(
        config: &TransferConfig,
        source_data: &HashMap<String, Vec<serde_json::Value>>,
        primary_keys: &HashMap<String, String>,
    ) -> ResolvedTransfer {
        let mut resolved = ResolvedTransfer::new(
            &config.name,
            &config.source_env,
            &config.target_env,
        );

        for entity_mapping in config.entity_mappings_by_priority() {
            let records = source_data
                .get(&entity_mapping.source_entity)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let pk_field = primary_keys
                .get(&entity_mapping.source_entity)
                .cloned()
                .unwrap_or_else(|| format!("{}id", entity_mapping.source_entity));

            let ctx = TransformContext {
                primary_key_field: pk_field,
            };

            let resolved_entity = Self::transform_entity(entity_mapping, records, &ctx);
            resolved.add_entity(resolved_entity);
        }

        resolved
    }

    /// Transform records for a single entity mapping
    pub fn transform_entity(
        mapping: &EntityMapping,
        source_records: &[serde_json::Value],
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
        resolved.set_field_names(field_names);

        for record in source_records {
            let resolved_record = Self::transform_record(record, &mapping.field_mappings, ctx);
            resolved.add_record(resolved_record);
        }

        resolved
    }

    /// Transform a single record
    pub fn transform_record(
        source: &serde_json::Value,
        field_mappings: &[FieldMapping],
        ctx: &TransformContext,
    ) -> ResolvedRecord {
        // Extract source ID
        let source_id = source
            .get(&ctx.primary_key_field)
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

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

        if errors.is_empty() {
            ResolvedRecord::upsert(source_id, fields)
        } else {
            let error_msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            ResolvedRecord::error_with_fields(source_id, fields, error_msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{Condition, FieldPath, Transform};
    use serde_json::json;

    fn make_ctx() -> TransformContext {
        TransformContext {
            primary_key_field: "accountid".to_string(),
        }
    }

    #[test]
    fn test_transform_record_success() {
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

        let result = TransformEngine::transform_record(&source, &mappings, &make_ctx());

        assert!(result.is_upsert());
        assert_eq!(result.get_field("name"), Some(&Value::String("Contoso".into())));
        assert_eq!(result.get_field("was_migrated"), Some(&Value::Bool(true)));
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

        let result = TransformEngine::transform_record(&source, &mappings, &make_ctx());

        assert!(result.is_error());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_transform_entity() {
        let records = vec![
            json!({
                "accountid": "a1b2c3d4-0000-0000-0000-000000000001",
                "name": "Contoso"
            }),
            json!({
                "accountid": "a1b2c3d4-0000-0000-0000-000000000002",
                "name": "Fabrikam"
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

        let result = TransformEngine::transform_entity(&mapping, &records, &make_ctx());

        assert_eq!(result.entity_name, "account");
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.upsert_count(), 2);
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

        let mut primary_keys = HashMap::new();
        primary_keys.insert("account".to_string(), "accountid".to_string());

        let result = TransformEngine::transform_all(&config, &source_data, &primary_keys);

        assert_eq!(result.config_name, "test-migration");
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.total_records(), 1);
    }
}
