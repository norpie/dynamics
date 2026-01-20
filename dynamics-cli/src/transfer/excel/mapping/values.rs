//! Value parsing and formatting for Excel cells

use crate::transfer::{Condition, DynamicValue, Fallback, Value};

/// Format a Value for Excel cell
pub fn format_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::DateTime(dt) => dt.to_rfc3339(),
        Value::Guid(g) => g.to_string(),
        Value::OptionSet(i) => i.to_string(),
        Value::Dynamic(d) => match d {
            DynamicValue::SourceValue => "$source".to_string(),
            DynamicValue::Now => "$now".to_string(),
            DynamicValue::NewGuid => "$guid".to_string(),
        },
    }
}

/// Parse an Excel cell string into a Value
pub fn parse_value(s: &str) -> Value {
    let s = s.trim();

    if s.is_empty() {
        return Value::Null;
    }

    // Check for dynamic values
    match s {
        "$source" => return Value::Dynamic(DynamicValue::SourceValue),
        "$now" => return Value::Dynamic(DynamicValue::Now),
        "$guid" => return Value::Dynamic(DynamicValue::NewGuid),
        _ => {}
    }

    // Check for boolean
    match s.to_lowercase().as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }

    // Check for optionset format: "123 (Label)" -> extract 123
    if let Some(int_val) = parse_optionset_format(s) {
        return Value::OptionSet(int_val);
    }

    // Try parsing as integer
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }

    // Try parsing as float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }

    // Try parsing as GUID
    if let Ok(g) = uuid::Uuid::parse_str(s) {
        return Value::Guid(g);
    }

    // Try parsing as datetime
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Value::DateTime(dt.with_timezone(&chrono::Utc));
    }

    // Default to string
    Value::String(s.to_string())
}

/// Parse optionset format: "123 (Label)" -> Some(123)
fn parse_optionset_format(s: &str) -> Option<i32> {
    // Match pattern: digits optionally followed by space and parentheses
    let s = s.trim();
    if let Some(paren_idx) = s.find('(') {
        let num_part = s[..paren_idx].trim();
        num_part.parse().ok()
    } else {
        None
    }
}

/// Format a condition operator for Excel
pub fn format_condition_op(condition: &Condition) -> &'static str {
    match condition {
        Condition::Equals { .. } => "eq",
        Condition::NotEquals { .. } => "neq",
        Condition::IsNull => "null",
        Condition::IsNotNull => "notnull",
    }
}

/// Get the comparison value from a condition (if applicable)
pub fn condition_value(condition: &Condition) -> Option<&Value> {
    match condition {
        Condition::Equals { value } => Some(value),
        Condition::NotEquals { value } => Some(value),
        Condition::IsNull | Condition::IsNotNull => None,
    }
}

/// Parse a condition from operator and value strings
pub fn parse_condition(op: &str, value_str: &str) -> Option<Condition> {
    let op = op.trim().to_lowercase();
    match op.as_str() {
        "eq" => Some(Condition::Equals {
            value: parse_value(value_str),
        }),
        "neq" => Some(Condition::NotEquals {
            value: parse_value(value_str),
        }),
        "null" => Some(Condition::IsNull),
        "notnull" => Some(Condition::IsNotNull),
        _ => None,
    }
}

/// Format a fallback type for Excel
pub fn format_fallback(fallback: &Fallback) -> &'static str {
    match fallback {
        Fallback::Error => "error",
        Fallback::Default { .. } => "default",
        Fallback::PassThrough => "passthrough",
        Fallback::Null => "null",
    }
}

/// Get the default value from a fallback (if applicable)
pub fn fallback_default_value(fallback: &Fallback) -> Option<&Value> {
    match fallback {
        Fallback::Default { value } => Some(value),
        _ => None,
    }
}

/// Parse a fallback from type and default value strings
pub fn parse_fallback(fallback_type: &str, default_str: &str) -> Fallback {
    let fallback_type = fallback_type.trim().to_lowercase();
    match fallback_type.as_str() {
        "error" => Fallback::Error,
        "default" => Fallback::Default {
            value: parse_value(default_str),
        },
        "passthrough" => Fallback::PassThrough,
        "null" => Fallback::Null,
        _ => Fallback::Error, // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_value_primitives() {
        assert_eq!(parse_value(""), Value::Null);
        assert_eq!(parse_value("true"), Value::Bool(true));
        assert_eq!(parse_value("false"), Value::Bool(false));
        assert_eq!(parse_value("42"), Value::Int(42));
        assert_eq!(parse_value("3.14"), Value::Float(3.14));
        assert_eq!(parse_value("hello"), Value::String("hello".into()));
    }

    #[test]
    fn test_parse_value_dynamic() {
        assert_eq!(
            parse_value("$source"),
            Value::Dynamic(DynamicValue::SourceValue)
        );
        assert_eq!(parse_value("$now"), Value::Dynamic(DynamicValue::Now));
        assert_eq!(parse_value("$guid"), Value::Dynamic(DynamicValue::NewGuid));
    }

    #[test]
    fn test_parse_optionset_format() {
        assert_eq!(parse_value("1 (Active)"), Value::OptionSet(1));
        assert_eq!(
            parse_value("100000000 (Custom)"),
            Value::OptionSet(100000000)
        );
        // Plain integers without parens should be Int, not OptionSet
        assert_eq!(parse_value("42"), Value::Int(42));
    }

    #[test]
    fn test_format_value_roundtrip() {
        let values = vec![
            Value::Bool(true),
            Value::Int(42),
            Value::String("test".into()),
            Value::Dynamic(DynamicValue::Now),
        ];

        for val in values {
            let formatted = format_value(&val);
            let parsed = parse_value(&formatted);
            assert_eq!(val, parsed);
        }
    }

    #[test]
    fn test_parse_condition() {
        assert!(matches!(
            parse_condition("eq", "42"),
            Some(Condition::Equals {
                value: Value::Int(42)
            })
        ));
        assert!(matches!(
            parse_condition("null", ""),
            Some(Condition::IsNull)
        ));
    }
}
