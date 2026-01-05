//! Field mapping validation
//!
//! Validates that field mappings are type-compatible and that constant values
//! can be parsed into the target field's type.

use crate::api::metadata::FieldType;

/// Validation result for a field mapping
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationResult {
    /// Mapping is valid
    Valid,
    /// Mapping might work at runtime (e.g., String→Integer depends on actual values)
    Warning(String),
    /// Mapping is definitely invalid
    Error(String),
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        !matches!(self, ValidationResult::Error(_))
    }

    pub fn is_error(&self) -> bool {
        matches!(self, ValidationResult::Error(_))
    }

    pub fn is_warning(&self) -> bool {
        matches!(self, ValidationResult::Warning(_))
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            ValidationResult::Valid => None,
            ValidationResult::Warning(msg) | ValidationResult::Error(msg) => Some(msg),
        }
    }
}

/// Try to parse a string value as the given field type
/// Returns Error if definitely invalid, Warning if might work, Valid if definitely valid
pub fn validate_constant_value(value: &str, target_type: &FieldType, is_required: bool) -> ValidationResult {
    let trimmed = value.trim();

    // Empty/null handling
    if trimmed.is_empty() || trimmed.to_lowercase() == "null" {
        return if is_required {
            ValidationResult::Error("Value required for this field".into())
        } else {
            ValidationResult::Valid // null is ok for optional fields
        };
    }

    match target_type {
        FieldType::String | FieldType::Memo => {
            // Any value is valid for string
            ValidationResult::Valid
        }

        FieldType::Boolean => {
            match trimmed.to_lowercase().as_str() {
                "true" | "false" | "1" | "0" | "yes" | "no" => ValidationResult::Valid,
                _ => ValidationResult::Error(format!("'{}' is not a valid boolean (use true/false)", trimmed)),
            }
        }

        FieldType::Integer | FieldType::OptionSet => {
            match trimmed.parse::<i64>() {
                Ok(_) => ValidationResult::Valid,
                Err(_) => ValidationResult::Error(format!("'{}' is not a valid integer", trimmed)),
            }
        }

        FieldType::MultiSelectOptionSet => {
            // Comma-separated list of integers (option values)
            let parts: Vec<&str> = trimmed.split(',').map(|s| s.trim()).collect();
            for part in parts {
                if part.is_empty() {
                    continue;
                }
                if part.parse::<i64>().is_err() {
                    return ValidationResult::Error(format!("'{}' is not a valid integer in multi-select", part));
                }
            }
            ValidationResult::Valid
        }

        FieldType::Decimal | FieldType::Money => {
            match trimmed.parse::<f64>() {
                Ok(_) => ValidationResult::Valid,
                Err(_) => ValidationResult::Error(format!("'{}' is not a valid decimal", trimmed)),
            }
        }

        FieldType::DateTime => {
            // Accept ISO 8601 formats and common date formats
            if parse_datetime(trimmed).is_some() {
                ValidationResult::Valid
            } else {
                ValidationResult::Error(format!("'{}' is not a valid date/time (use ISO 8601 format)", trimmed))
            }
        }

        FieldType::UniqueIdentifier | FieldType::Lookup => {
            // GUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
            if is_valid_guid(trimmed) {
                ValidationResult::Valid
            } else {
                ValidationResult::Error(format!("'{}' is not a valid GUID", trimmed))
            }
        }

        FieldType::Other(_) => {
            // Can't validate unknown types
            ValidationResult::Warning("Unknown field type - cannot validate".into())
        }
    }
}

/// Check type compatibility for copy operations
/// Returns the validation result for copying from source_type to target_type
pub fn validate_copy_types(source_type: &FieldType, target_type: &FieldType) -> ValidationResult {
    use FieldType::*;

    // Same type is always valid
    if source_type == target_type {
        return ValidationResult::Valid;
    }

    match (source_type, target_type) {
        // Anything can become a string
        (_, String) | (_, Memo) => ValidationResult::Valid,

        // Numeric promotions (safe)
        (Integer, Decimal) | (Integer, Money) => ValidationResult::Valid,
        (OptionSet, Integer) | (Integer, OptionSet) => ValidationResult::Valid,

        // Numeric to numeric (might lose precision or fail)
        (Decimal, Integer) | (Money, Integer) => {
            ValidationResult::Warning("Decimal to integer may truncate".into())
        }
        (Decimal, Money) | (Money, Decimal) => ValidationResult::Valid,

        // String to anything - depends on actual values
        (String, Integer) | (Memo, Integer) | (String, OptionSet) | (Memo, OptionSet) => {
            ValidationResult::Warning("String to integer requires parseable values".into())
        }
        (String, Decimal) | (Memo, Decimal) | (String, Money) | (Memo, Money) => {
            ValidationResult::Warning("String to decimal requires parseable values".into())
        }
        (String, Boolean) | (Memo, Boolean) => {
            ValidationResult::Warning("String to boolean requires 'true'/'false' values".into())
        }
        (String, DateTime) | (Memo, DateTime) => {
            ValidationResult::Warning("String to datetime requires ISO 8601 format".into())
        }
        (String, UniqueIdentifier) | (Memo, UniqueIdentifier) => {
            ValidationResult::Warning("String to GUID requires valid GUID format".into())
        }
        (String, Lookup) | (Memo, Lookup) => {
            ValidationResult::Warning("String to lookup requires valid GUID format".into())
        }

        // Boolean conversions
        (Boolean, Integer) | (Boolean, OptionSet) => {
            ValidationResult::Warning("Boolean to integer: true=1, false=0".into())
        }
        (Integer, Boolean) | (OptionSet, Boolean) => {
            ValidationResult::Warning("Integer to boolean: 0=false, non-zero=true".into())
        }

        // GUID/Lookup conversions
        (UniqueIdentifier, Lookup) | (Lookup, UniqueIdentifier) => ValidationResult::Valid,
        (Lookup, Lookup) => {
            // Different lookup targets - needs ID remapping at runtime
            ValidationResult::Warning("Lookup to lookup may need ID remapping".into())
        }

        // DateTime conversions
        (DateTime, String) | (DateTime, Memo) => ValidationResult::Valid,

        // Invalid conversions
        (DateTime, Integer) | (DateTime, Decimal) | (DateTime, Boolean) |
        (DateTime, Money) | (DateTime, OptionSet) | (DateTime, UniqueIdentifier) | (DateTime, Lookup) => {
            ValidationResult::Error("DateTime cannot be converted to this type".into())
        }

        (Boolean, Decimal) | (Boolean, Money) | (Boolean, DateTime) |
        (Boolean, UniqueIdentifier) | (Boolean, Lookup) => {
            ValidationResult::Error("Boolean cannot be converted to this type".into())
        }

        (Integer, DateTime) | (Decimal, DateTime) | (Money, DateTime) |
        (OptionSet, DateTime) => {
            ValidationResult::Error("Numeric types cannot be converted to DateTime".into())
        }

        (Integer, UniqueIdentifier) | (Integer, Lookup) |
        (Decimal, UniqueIdentifier) | (Decimal, Lookup) |
        (Money, UniqueIdentifier) | (Money, Lookup) |
        (OptionSet, UniqueIdentifier) | (OptionSet, Lookup) => {
            ValidationResult::Error("Numeric types cannot be converted to GUID/Lookup".into())
        }

        (Decimal, Boolean) | (Money, Boolean) => {
            ValidationResult::Error("Decimal cannot be converted to boolean".into())
        }

        (Decimal, OptionSet) | (Money, OptionSet) => {
            ValidationResult::Error("Decimal cannot be converted to OptionSet".into())
        }

        (UniqueIdentifier, Integer) | (UniqueIdentifier, Decimal) | (UniqueIdentifier, Boolean) |
        (UniqueIdentifier, DateTime) | (UniqueIdentifier, Money) | (UniqueIdentifier, OptionSet) |
        (Lookup, Integer) | (Lookup, Decimal) | (Lookup, Boolean) |
        (Lookup, DateTime) | (Lookup, Money) | (Lookup, OptionSet) => {
            ValidationResult::Error("GUID/Lookup cannot be converted to this type".into())
        }

        // Other/unknown types
        (Other(_), _) | (_, Other(_)) => {
            ValidationResult::Warning("Unknown field type - cannot validate compatibility".into())
        }

        // Catch-all for any missed combinations
        _ => ValidationResult::Warning("Type compatibility uncertain".into())
    }
}

/// Check if using Copy transform on an OptionSet field that might need ValueMap
/// Returns a warning if the source field is OptionSet/MultiSelectOptionSet type
pub fn validate_optionset_copy(
    source_type: &FieldType,
    target_type: &FieldType,
) -> ValidationResult {
    // Only warn for OptionSet fields mapping to OptionSet
    if matches!(source_type, FieldType::OptionSet | FieldType::MultiSelectOptionSet)
        && matches!(target_type, FieldType::OptionSet | FieldType::MultiSelectOptionSet)
    {
        ValidationResult::Warning(
            "OptionSet field: Consider using ValueMap to ensure values are mapped correctly".into()
        )
    } else {
        ValidationResult::Valid
    }
}

/// Parse a datetime string - accepts various formats
fn parse_datetime(s: &str) -> Option<()> {
    // ISO 8601 formats
    let formats = [
        "%Y-%m-%dT%H:%M:%S%.fZ",      // 2024-01-15T10:30:00.000Z
        "%Y-%m-%dT%H:%M:%SZ",          // 2024-01-15T10:30:00Z
        "%Y-%m-%dT%H:%M:%S",           // 2024-01-15T10:30:00
        "%Y-%m-%d %H:%M:%S",           // 2024-01-15 10:30:00
        "%Y-%m-%d",                    // 2024-01-15
        "%d/%m/%Y",                    // 15/01/2024
        "%m/%d/%Y",                    // 01/15/2024
    ];

    for format in formats {
        if chrono::NaiveDateTime::parse_from_str(s, format).is_ok() {
            return Some(());
        }
        if chrono::NaiveDate::parse_from_str(s, format).is_ok() {
            return Some(());
        }
    }

    // Also try parsing as DateTime with timezone
    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
        return Some(());
    }

    None
}

/// Check if a string is a valid GUID
fn is_valid_guid(s: &str) -> bool {
    // Standard GUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    // Also accept with or without braces
    let s = s.trim_start_matches('{').trim_end_matches('}');

    if s.len() != 36 {
        return false;
    }

    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }

    let expected_lengths = [8, 4, 4, 4, 12];
    for (part, expected_len) in parts.iter().zip(expected_lengths.iter()) {
        if part.len() != *expected_len {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boolean_validation() {
        assert!(validate_constant_value("true", &FieldType::Boolean, false).is_valid());
        assert!(validate_constant_value("false", &FieldType::Boolean, false).is_valid());
        assert!(validate_constant_value("1", &FieldType::Boolean, false).is_valid());
        assert!(validate_constant_value("0", &FieldType::Boolean, false).is_valid());
        assert!(validate_constant_value("maybe", &FieldType::Boolean, false).is_error());
    }

    #[test]
    fn test_integer_validation() {
        assert!(validate_constant_value("42", &FieldType::Integer, false).is_valid());
        assert!(validate_constant_value("-100", &FieldType::Integer, false).is_valid());
        assert!(validate_constant_value("3.14", &FieldType::Integer, false).is_error());
        assert!(validate_constant_value("abc", &FieldType::Integer, false).is_error());
    }

    #[test]
    fn test_guid_validation() {
        assert!(is_valid_guid("12345678-1234-1234-1234-123456789abc"));
        assert!(is_valid_guid("{12345678-1234-1234-1234-123456789abc}"));
        assert!(!is_valid_guid("not-a-guid"));
        assert!(!is_valid_guid("12345678-1234-1234-1234"));
    }

    #[test]
    fn test_copy_type_compatibility() {
        // Same type
        assert!(validate_copy_types(&FieldType::String, &FieldType::String).is_valid());

        // Safe promotions
        assert!(validate_copy_types(&FieldType::Integer, &FieldType::Decimal).is_valid());
        assert!(validate_copy_types(&FieldType::Integer, &FieldType::String).is_valid());

        // Warnings
        assert!(validate_copy_types(&FieldType::String, &FieldType::Integer).is_warning());
        assert!(validate_copy_types(&FieldType::Decimal, &FieldType::Integer).is_warning());

        // Errors
        assert!(validate_copy_types(&FieldType::DateTime, &FieldType::Boolean).is_error());
        assert!(validate_copy_types(&FieldType::Boolean, &FieldType::Lookup).is_error());
    }
}
