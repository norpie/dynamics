//! Dataverse value representation for transfers

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A value in Dataverse, used for transform inputs and outputs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    /// Null/empty value
    Null,
    /// String value
    String(String),
    /// Whole number (integer)
    Int(i64),
    /// Floating point (decimal, money, float)
    Float(f64),
    /// Boolean (Two Options)
    Bool(bool),
    /// Date and time
    DateTime(DateTime<Utc>),
    /// Unique identifier
    Guid(Uuid),
    /// Option set value (stored as integer)
    OptionSet(i32),
    /// Dynamic value resolved at transform time
    Dynamic(DynamicValue),
}

/// Dynamic values resolved at transform execution time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DynamicValue {
    /// Use the source field value (passthrough)
    SourceValue,
    /// Current timestamp at execution
    Now,
    /// Generate a new GUID
    NewGuid,
}

impl Value {
    /// Check if this value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if this is a dynamic value
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Value::Dynamic(_))
    }

    /// Try to get as string
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as integer
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::OptionSet(i) => Some(*i as i64),
            _ => None,
        }
    }

    /// Try to get as float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as GUID
    pub fn as_guid(&self) -> Option<Uuid> {
        match self {
            Value::Guid(g) => Some(*g),
            _ => None,
        }
    }

    /// Convert to JSON value for API calls
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Int(i) => serde_json::json!(*i),
            Value::Float(f) => serde_json::json!(*f),
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
            Value::Guid(g) => serde_json::Value::String(g.to_string()),
            Value::OptionSet(i) => serde_json::json!(*i),
            Value::Dynamic(_) => {
                panic!("Dynamic values must be resolved before converting to JSON")
            }
        }
    }

    /// Parse from JSON value
    pub fn from_json(json: &serde_json::Value) -> Self {
        match json {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => {
                // Try to parse as GUID
                if let Ok(guid) = Uuid::parse_str(s) {
                    return Value::Guid(guid);
                }
                // Try to parse as DateTime
                if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                    return Value::DateTime(dt.with_timezone(&Utc));
                }
                Value::String(s.clone())
            }
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                // Complex types not directly supported
                Value::String(json.to_string())
            }
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "(null)"),
            Value::String(s) => write!(f, "{}", s),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Bool(b) => write!(f, "{}", b),
            Value::DateTime(dt) => write!(f, "{}", dt.to_rfc3339()),
            Value::Guid(g) => write!(f, "{}", g),
            Value::OptionSet(i) => write!(f, "{}", i),
            Value::Dynamic(d) => write!(f, "{}", d),
        }
    }
}

impl std::fmt::Display for DynamicValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DynamicValue::SourceValue => write!(f, "$source"),
            DynamicValue::Now => write!(f, "$now"),
            DynamicValue::NewGuid => write!(f, "$guid"),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}
