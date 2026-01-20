//! Transform application logic

use chrono::Utc;
use uuid::Uuid;

use crate::transfer::{Condition, DynamicValue, Fallback, ResolverContext, Transform, Value};

use super::path::resolve_path;

/// Result of applying a transform
pub type TransformResult = Result<Value, String>;

/// Apply a transform to a source record
///
/// # Arguments
/// * `transform` - The transform to apply
/// * `record` - The source record (JSON)
/// * `resolver_ctx` - Optional resolver context for Copy transforms with resolvers
///
/// When a Copy transform has a resolver specified:
/// - For single-field resolvers: the source_path value is used for lookup
/// - For compound key resolvers: the resolver's match_fields define where to get values from
pub fn apply_transform(
    transform: &Transform,
    record: &serde_json::Value,
    resolver_ctx: Option<&ResolverContext>,
) -> TransformResult {
    match transform {
        Transform::Copy {
            source_path,
            resolver,
        } => {
            // If a resolver is specified, resolve using the resolver
            if let Some(resolver_name) = resolver {
                match resolver_ctx {
                    Some(ctx) => {
                        // Get the resolver's match fields
                        let Some(match_fields) = ctx.get_match_fields(resolver_name) else {
                            return Err(format!(
                                "Resolver '{}' not found in context",
                                resolver_name
                            ));
                        };

                        // Both single-field and compound key resolvers use MatchField source_paths
                        // The Transform's source_path is for the final value, MatchField source_paths are for lookup matching
                        if match_fields.len() == 1 {
                            // Single-field: use the MatchField's source_path (NOT the Transform's source_path!)
                            // Transform source_path might point to expanded object, but MatchField source_path
                            // points to the specific field value we need for matching
                            let value = resolve_path(record, &match_fields[0].source_path);
                            let json_value = value.to_json();
                            let target_field = match_fields[0].target_field.as_str();
                            let pairs = [(target_field, &json_value)];
                            ctx.resolve_composite_to_value(resolver_name, &pairs)
                        } else {
                            // Compound key: use resolver's match_fields to get values
                            let mut pairs: Vec<(&str, serde_json::Value)> =
                                Vec::with_capacity(match_fields.len());

                            // Log source record keys once for debugging
                            static LOGGED_KEYS: std::sync::atomic::AtomicBool =
                                std::sync::atomic::AtomicBool::new(false);
                            if !LOGGED_KEYS.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                if let Some(obj) = record.as_object() {
                                    let keys: Vec<_> = obj.keys().collect();
                                    log::info!(
                                        "Compound resolver '{}': source record has {} keys: {:?}",
                                        resolver_name,
                                        keys.len(),
                                        keys
                                    );
                                }
                            }

                            for mf in match_fields {
                                let value = resolve_path(record, &mf.source_path);
                                log::trace!(
                                    "Compound resolver: source_path '{}' -> {:?}",
                                    mf.source_path,
                                    value
                                );
                                pairs.push((mf.target_field.as_str(), value.to_json()));
                            }

                            // Build reference pairs for the resolver
                            let ref_pairs: Vec<(&str, &serde_json::Value)> =
                                pairs.iter().map(|(k, v)| (*k, v)).collect();

                            ctx.resolve_composite_to_value(resolver_name, &ref_pairs)
                        }
                    }
                    None => Err(format!(
                        "Resolver '{}' specified but no resolver context available",
                        resolver_name
                    )),
                }
            } else {
                // No resolver - just copy the value
                Ok(resolve_path(record, source_path))
            }
        }

        Transform::Constant { value } => resolve_dynamic(value),

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

        Transform::Format {
            template,
            null_handling,
        } => {
            let result = super::format::evaluate(template, record, *null_handling)?;
            Ok(Value::String(result))
        }

        Transform::Replace {
            source_path,
            replacements,
        } => {
            let source_value = resolve_path(record, source_path);
            match source_value {
                Value::String(s) => {
                    let mut result = s;
                    for r in replacements {
                        if r.is_regex {
                            match regex::Regex::new(&r.pattern) {
                                Ok(re) => {
                                    result = re
                                        .replace_all(&result, r.replacement.as_str())
                                        .into_owned();
                                }
                                Err(e) => {
                                    return Err(format!(
                                        "Invalid regex pattern '{}': {}",
                                        r.pattern, e
                                    ));
                                }
                            }
                        } else {
                            result = result.replace(&r.pattern, &r.replacement);
                        }
                    }
                    Ok(Value::String(result))
                }
                Value::Null => Ok(Value::Null),
                other => Err(format!(
                    "Replace transform requires string value, got: {}",
                    other
                )),
            }
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
        Fallback::Error => Err(format!("No mapping found for value: {}", source_value)),
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
            resolver: None,
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("Contoso".into()));
    }

    #[test]
    fn test_apply_constant() {
        let record = json!({});
        let transform = Transform::Constant {
            value: Value::Bool(true),
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_apply_constant_dynamic_now() {
        let record = json!({});
        let transform = Transform::Constant {
            value: Value::Dynamic(DynamicValue::Now),
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert!(matches!(result, Value::DateTime(_)));
    }

    #[test]
    fn test_apply_constant_dynamic_guid() {
        let record = json!({});
        let transform = Transform::Constant {
            value: Value::Dynamic(DynamicValue::NewGuid),
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert!(matches!(result, Value::Guid(_)));
    }

    #[test]
    fn test_apply_conditional_true() {
        let record = json!({"statecode": 0});
        let transform = Transform::Conditional {
            source_path: FieldPath::simple("statecode"),
            condition: Condition::Equals {
                value: Value::Int(0),
            },
            then_value: Value::String("Active".into()),
            else_value: Value::String("Inactive".into()),
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("Active".into()));
    }

    #[test]
    fn test_apply_conditional_false() {
        let record = json!({"statecode": 1});
        let transform = Transform::Conditional {
            source_path: FieldPath::simple("statecode"),
            condition: Condition::Equals {
                value: Value::Int(0),
            },
            then_value: Value::String("Active".into()),
            else_value: Value::String("Inactive".into()),
        };

        let result = apply_transform(&transform, &record, None).unwrap();
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

        let result = apply_transform(&transform, &record, None).unwrap();
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

        let result = apply_transform(&transform, &record, None).unwrap();
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
            fallback: Fallback::Default {
                value: Value::Int(0),
            },
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn test_apply_value_map_fallback_passthrough() {
        let record = json!({"gendercode": 99});
        let transform = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![(Value::Int(1), Value::Int(100))],
            fallback: Fallback::PassThrough,
        };

        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_apply_value_map_fallback_error() {
        let record = json!({"gendercode": 99});
        let transform = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![(Value::Int(1), Value::Int(100))],
            fallback: Fallback::Error,
        };

        let result = apply_transform(&transform, &record, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_replace_single() {
        use crate::transfer::Replacement;
        let record = json!({"name": "Hello World"});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![Replacement::literal("World", "Universe")],
        };
        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("Hello Universe".into()));
    }

    #[test]
    fn test_apply_replace_multiple_in_order() {
        use crate::transfer::Replacement;
        let record = json!({"name": "foo bar baz"});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![
                Replacement::literal("foo", "bar"), // "bar bar baz"
                Replacement::literal("bar", "qux"), // "qux qux baz"
            ],
        };
        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("qux qux baz".into()));
    }

    #[test]
    fn test_apply_replace_null() {
        use crate::transfer::Replacement;
        let record = json!({"name": null});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![Replacement::literal("a", "b")],
        };
        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_apply_replace_non_string_error() {
        use crate::transfer::Replacement;
        let record = json!({"count": 42});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("count"),
            replacements: vec![Replacement::literal("a", "b")],
        };
        let result = apply_transform(&transform, &record, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_replace_regex() {
        use crate::transfer::Replacement;
        let record = json!({"name": "Hello123World456"});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![Replacement::regex(r"\d+", "_")],
        };
        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("Hello_World_".into()));
    }

    #[test]
    fn test_apply_replace_regex_capture_groups() {
        use crate::transfer::Replacement;
        let record = json!({"name": "John Smith"});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![Replacement::regex(r"(\w+) (\w+)", "$2, $1")],
        };
        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("Smith, John".into()));
    }

    #[test]
    fn test_apply_replace_invalid_regex() {
        use crate::transfer::Replacement;
        let record = json!({"name": "test"});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![Replacement::regex(r"[invalid", "x")],
        };
        let result = apply_transform(&transform, &record, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid regex pattern"));
    }

    #[test]
    fn test_apply_replace_mixed_literal_and_regex() {
        use crate::transfer::Replacement;
        let record = json!({"name": "Hello123 World456"});
        let transform = Transform::Replace {
            source_path: FieldPath::simple("name"),
            replacements: vec![
                Replacement::literal("Hello", "Hi"),
                Replacement::regex(r"\d+", ""),
            ],
        };
        let result = apply_transform(&transform, &record, None).unwrap();
        assert_eq!(result, Value::String("Hi World".into()));
    }
}
