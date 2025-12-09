//! Transform application logic

use chrono::Utc;
use uuid::Uuid;

use crate::transfer::{Condition, DynamicValue, Fallback, Transform, Value};

use super::path::resolve_path;

/// Result of applying a transform
pub type TransformResult = Result<Value, String>;

/// Apply a transform to a source record
pub fn apply_transform(transform: &Transform, record: &serde_json::Value) -> TransformResult {
    match transform {
        Transform::Copy { source_path } => {
            let value = resolve_path(record, source_path);
            Ok(value)
        }

        Transform::Constant { value } => {
            resolve_dynamic(value)
        }

        Transform::Conditional {
            source_path,
            condition,
            then_value,
            else_value,
        } => {
            let source_value = resolve_path(record, source_path);
            let result_value = if evaluate_condition(condition, &source_value) {
                then_value
            } else {
                else_value
            };
            resolve_dynamic(result_value)
        }

        Transform::ValueMap {
            source_path,
            mappings,
            fallback,
        } => {
            let source_value = resolve_path(record, source_path);

            // Look for a matching mapping
            for (from, to) in mappings {
                if values_equal(&source_value, from) {
                    return resolve_dynamic(to);
                }
            }

            // No match found, apply fallback
            apply_fallback(fallback, source_value)
        }
    }
}

/// Resolve dynamic values like $now, $guid, $source
fn resolve_dynamic(value: &Value) -> TransformResult {
    match value {
        Value::Dynamic(dyn_val) => match dyn_val {
            DynamicValue::Now => Ok(Value::DateTime(Utc::now())),
            DynamicValue::NewGuid => Ok(Value::Guid(Uuid::new_v4())),
            DynamicValue::SourceValue => {
                // SourceValue should only appear in fallback context
                // where we already have the source value
                Err("$source can only be used in fallback context".to_string())
            }
        },
        _ => Ok(value.clone()),
    }
}

/// Evaluate a condition against a value
fn evaluate_condition(condition: &Condition, value: &Value) -> bool {
    condition.evaluate(value)
}

/// Compare two values for equality (used in value maps)
fn values_equal(a: &Value, b: &Value) -> bool {
    a == b
}

/// Apply fallback behavior when no value map entry matches
fn apply_fallback(fallback: &Fallback, source_value: Value) -> TransformResult {
    match fallback {
        Fallback::Error => {
            Err(format!("No mapping found for value: {}", source_value))
        }
        Fallback::Default { value } => resolve_dynamic(value),
        Fallback::PassThrough => Ok(source_value),
        Fallback::Null => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::FieldPath;
    use serde_json::json;

    #[test]
    fn test_apply_copy() {
        let record = json!({"name": "Contoso", "revenue": 500000});
        let transform = Transform::Copy {
            source_path: FieldPath::simple("name"),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::String("Contoso".into()));
    }

    #[test]
    fn test_apply_constant() {
        let record = json!({});
        let transform = Transform::Constant {
            value: Value::Bool(true),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_apply_constant_dynamic_now() {
        let record = json!({});
        let transform = Transform::Constant {
            value: Value::Dynamic(DynamicValue::Now),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert!(matches!(result, Value::DateTime(_)));
    }

    #[test]
    fn test_apply_constant_dynamic_guid() {
        let record = json!({});
        let transform = Transform::Constant {
            value: Value::Dynamic(DynamicValue::NewGuid),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert!(matches!(result, Value::Guid(_)));
    }

    #[test]
    fn test_apply_conditional_true() {
        let record = json!({"statecode": 0});
        let transform = Transform::Conditional {
            source_path: FieldPath::simple("statecode"),
            condition: Condition::Equals { value: Value::Int(0) },
            then_value: Value::String("Active".into()),
            else_value: Value::String("Inactive".into()),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::String("Active".into()));
    }

    #[test]
    fn test_apply_conditional_false() {
        let record = json!({"statecode": 1});
        let transform = Transform::Conditional {
            source_path: FieldPath::simple("statecode"),
            condition: Condition::Equals { value: Value::Int(0) },
            then_value: Value::String("Active".into()),
            else_value: Value::String("Inactive".into()),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::String("Inactive".into()));
    }

    #[test]
    fn test_apply_conditional_is_null() {
        let record = json!({"optionalfield": null});
        let transform = Transform::Conditional {
            source_path: FieldPath::simple("optionalfield"),
            condition: Condition::IsNull,
            then_value: Value::String("N/A".into()),
            else_value: Value::Dynamic(DynamicValue::SourceValue),
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::String("N/A".into()));
    }

    #[test]
    fn test_apply_value_map_match() {
        let record = json!({"gendercode": 1});
        let transform = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![
                (Value::Int(1), Value::Int(100)),
                (Value::Int(2), Value::Int(200)),
            ],
            fallback: Fallback::Error,
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::Int(100));
    }

    #[test]
    fn test_apply_value_map_fallback_default() {
        let record = json!({"gendercode": 99});
        let transform = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![
                (Value::Int(1), Value::Int(100)),
                (Value::Int(2), Value::Int(200)),
            ],
            fallback: Fallback::Default { value: Value::Int(0) },
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn test_apply_value_map_fallback_passthrough() {
        let record = json!({"gendercode": 99});
        let transform = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![
                (Value::Int(1), Value::Int(100)),
            ],
            fallback: Fallback::PassThrough,
        };

        let result = apply_transform(&transform, &record).unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_apply_value_map_fallback_error() {
        let record = json!({"gendercode": 99});
        let transform = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![
                (Value::Int(1), Value::Int(100)),
            ],
            fallback: Fallback::Error,
        };

        let result = apply_transform(&transform, &record);
        assert!(result.is_err());
    }
}
