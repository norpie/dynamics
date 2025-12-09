//! Transform definitions for field mappings

use serde::{Deserialize, Serialize};

use super::Value;

/// A transform that produces a target field value from source data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Transform {
    /// Direct field copy, optionally traversing a lookup
    Copy {
        /// Source field path (e.g., "name" or "accountid.name")
        source_path: FieldPath,
    },
    /// Static constant value
    Constant {
        /// The constant value to use
        value: Value,
    },
    /// Conditional transform with single if/else
    Conditional {
        /// Source field to evaluate
        source_path: FieldPath,
        /// Condition to check
        condition: Condition,
        /// Value if condition is true
        then_value: Value,
        /// Value if condition is false
        else_value: Value,
    },
    /// Value mapping lookup table
    ValueMap {
        /// Source field to map from
        source_path: FieldPath,
        /// Mapping entries (source_value -> target_value)
        mappings: Vec<(Value, Value)>,
        /// Fallback behavior when no mapping matches
        fallback: Fallback,
    },
}

impl Transform {
    /// Create a simple copy transform
    pub fn copy(source_field: &str) -> Result<Self, FieldPathError> {
        Ok(Transform::Copy {
            source_path: FieldPath::parse(source_field)?,
        })
    }

    /// Create a constant transform
    pub fn constant(value: Value) -> Self {
        Transform::Constant { value }
    }

    /// Get a human-readable description of this transform
    pub fn describe(&self) -> String {
        match self {
            Transform::Copy { source_path } => {
                format!("copy({})", source_path)
            }
            Transform::Constant { value } => {
                format!("constant({})", value)
            }
            Transform::Conditional {
                source_path,
                condition,
                then_value,
                else_value,
            } => {
                format!(
                    "if({} {}) then {} else {}",
                    source_path, condition, then_value, else_value
                )
            }
            Transform::ValueMap {
                source_path,
                mappings,
                ..
            } => {
                format!("map({}) [{} entries]", source_path, mappings.len())
            }
        }
    }

    /// Get the source field(s) required by this transform
    /// Returns the base field name (first segment of path)
    pub fn source_fields(&self) -> Vec<&str> {
        match self {
            Transform::Copy { source_path } => vec![source_path.base_field()],
            Transform::Constant { .. } => vec![],
            Transform::Conditional { source_path, .. } => vec![source_path.base_field()],
            Transform::ValueMap { source_path, .. } => vec![source_path.base_field()],
        }
    }
}

/// A path to a field, optionally traversing a lookup relationship
///
/// Examples:
/// - "name" -> single field
/// - "accountid.name" -> lookup traversal (accountid lookup -> name field)
///
/// Limited to at most one level of traversal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldPath {
    /// The field segments (1 or 2 elements)
    segments: Vec<String>,
}

/// Error when parsing a field path
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPathError {
    /// Path is empty
    Empty,
    /// Path has too many segments (max 2 allowed)
    TooManySegments { count: usize },
    /// Segment is empty
    EmptySegment,
}

impl std::fmt::Display for FieldPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldPathError::Empty => write!(f, "field path cannot be empty"),
            FieldPathError::TooManySegments { count } => {
                write!(
                    f,
                    "field path has {} segments, maximum is 2 (e.g., 'accountid.name')",
                    count
                )
            }
            FieldPathError::EmptySegment => write!(f, "field path contains empty segment"),
        }
    }
}

impl std::error::Error for FieldPathError {}

impl FieldPath {
    /// Parse a field path from a string
    ///
    /// Validates that:
    /// - Path is not empty
    /// - At most 2 segments (one dot)
    /// - No empty segments
    pub fn parse(path: &str) -> Result<Self, FieldPathError> {
        if path.is_empty() {
            return Err(FieldPathError::Empty);
        }

        let segments: Vec<String> = path.split('.').map(|s| s.to_string()).collect();

        if segments.len() > 2 {
            return Err(FieldPathError::TooManySegments {
                count: segments.len(),
            });
        }

        if segments.iter().any(|s| s.is_empty()) {
            return Err(FieldPathError::EmptySegment);
        }

        Ok(FieldPath { segments })
    }

    /// Create a simple single-field path (no validation needed)
    pub fn simple(field: impl Into<String>) -> Self {
        FieldPath {
            segments: vec![field.into()],
        }
    }

    /// Create a lookup traversal path
    pub fn lookup(lookup_field: impl Into<String>, target_field: impl Into<String>) -> Self {
        FieldPath {
            segments: vec![lookup_field.into(), target_field.into()],
        }
    }

    /// Check if this path traverses a lookup
    pub fn is_lookup_traversal(&self) -> bool {
        self.segments.len() == 2
    }

    /// Get the base field name (first segment)
    pub fn base_field(&self) -> &str {
        &self.segments[0]
    }

    /// Get the target field name for lookup traversal (second segment)
    pub fn lookup_field(&self) -> Option<&str> {
        self.segments.get(1).map(|s| s.as_str())
    }

    /// Get all segments
    pub fn segments(&self) -> &[String] {
        &self.segments
    }
}

impl std::fmt::Display for FieldPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.segments.join("."))
    }
}

impl TryFrom<&str> for FieldPath {
    type Error = FieldPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        FieldPath::parse(value)
    }
}

impl TryFrom<String> for FieldPath {
    type Error = FieldPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        FieldPath::parse(&value)
    }
}

/// Condition for conditional transforms
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Condition {
    /// Equals comparison
    Equals { value: Value },
    /// Not equals comparison
    NotEquals { value: Value },
    /// Is null check
    IsNull,
    /// Is not null check
    IsNotNull,
}

impl Condition {
    /// Evaluate this condition against a value
    pub fn evaluate(&self, actual: &Value) -> bool {
        match self {
            Condition::Equals { value } => actual == value,
            Condition::NotEquals { value } => actual != value,
            Condition::IsNull => actual.is_null(),
            Condition::IsNotNull => !actual.is_null(),
        }
    }
}

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Condition::Equals { value } => write!(f, "= {}", value),
            Condition::NotEquals { value } => write!(f, "!= {}", value),
            Condition::IsNull => write!(f, "is null"),
            Condition::IsNotNull => write!(f, "is not null"),
        }
    }
}

/// Fallback behavior for value maps when no mapping matches
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Fallback {
    /// Raise an error (transform fails)
    Error,
    /// Use a default value
    Default { value: Value },
    /// Pass through the source value unchanged
    PassThrough,
    /// Use null
    Null,
}

impl Default for Fallback {
    fn default() -> Self {
        Fallback::Error
    }
}

impl std::fmt::Display for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fallback::Error => write!(f, "error"),
            Fallback::Default { value } => write!(f, "default({})", value),
            Fallback::PassThrough => write!(f, "passthrough"),
            Fallback::Null => write!(f, "null"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_path_simple() {
        let path = FieldPath::parse("name").unwrap();
        assert_eq!(path.base_field(), "name");
        assert!(!path.is_lookup_traversal());
        assert_eq!(path.lookup_field(), None);
        assert_eq!(path.to_string(), "name");
    }

    #[test]
    fn test_field_path_lookup() {
        let path = FieldPath::parse("accountid.name").unwrap();
        assert_eq!(path.base_field(), "accountid");
        assert!(path.is_lookup_traversal());
        assert_eq!(path.lookup_field(), Some("name"));
        assert_eq!(path.to_string(), "accountid.name");
    }

    #[test]
    fn test_field_path_too_many_segments() {
        let result = FieldPath::parse("a.b.c");
        assert!(matches!(
            result,
            Err(FieldPathError::TooManySegments { count: 3 })
        ));
    }

    #[test]
    fn test_field_path_empty() {
        assert!(matches!(FieldPath::parse(""), Err(FieldPathError::Empty)));
    }

    #[test]
    fn test_field_path_empty_segment() {
        assert!(matches!(
            FieldPath::parse("accountid."),
            Err(FieldPathError::EmptySegment)
        ));
        assert!(matches!(
            FieldPath::parse(".name"),
            Err(FieldPathError::EmptySegment)
        ));
    }

    #[test]
    fn test_condition_evaluate() {
        let value = Value::Int(42);

        assert!(Condition::Equals {
            value: Value::Int(42)
        }
        .evaluate(&value));
        assert!(!Condition::Equals {
            value: Value::Int(0)
        }
        .evaluate(&value));

        assert!(Condition::NotEquals {
            value: Value::Int(0)
        }
        .evaluate(&value));
        assert!(!Condition::NotEquals {
            value: Value::Int(42)
        }
        .evaluate(&value));

        assert!(!Condition::IsNull.evaluate(&value));
        assert!(Condition::IsNotNull.evaluate(&value));

        assert!(Condition::IsNull.evaluate(&Value::Null));
        assert!(!Condition::IsNotNull.evaluate(&Value::Null));
    }

    #[test]
    fn test_transform_describe() {
        let copy = Transform::copy("name").unwrap();
        assert_eq!(copy.describe(), "copy(name)");

        let constant = Transform::constant(Value::Bool(true));
        assert_eq!(constant.describe(), "constant(true)");

        let conditional = Transform::Conditional {
            source_path: FieldPath::simple("statecode"),
            condition: Condition::Equals {
                value: Value::Int(0),
            },
            then_value: Value::Int(1),
            else_value: Value::Int(2),
        };
        assert_eq!(conditional.describe(), "if(statecode = 0) then 1 else 2");

        let value_map = Transform::ValueMap {
            source_path: FieldPath::simple("gendercode"),
            mappings: vec![
                (Value::Int(1), Value::Int(100)),
                (Value::Int(2), Value::Int(200)),
            ],
            fallback: Fallback::Null,
        };
        assert_eq!(value_map.describe(), "map(gendercode) [2 entries]");
    }
}
