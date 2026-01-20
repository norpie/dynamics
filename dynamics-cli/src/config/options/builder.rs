//! Fluent builder API for creating option definitions

use super::types::{OptionDefinition, OptionType, OptionValue};
use anyhow::Result;

/// Builder for creating option definitions with a fluent API
pub struct OptionDefBuilder {
    namespace: String,
    local_key: String,
    display_name: Option<String>,
    description: Option<String>,
    ty: Option<OptionType>,
    default: Option<OptionValue>,
}

impl OptionDefBuilder {
    /// Create a new builder for an option in a namespace
    pub fn new(namespace: &str, local_key: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            local_key: local_key.to_string(),
            display_name: None,
            description: None,
            ty: None,
            default: None,
        }
    }

    /// Set the display name (human-readable label)
    pub fn display_name(mut self, name: &str) -> Self {
        self.display_name = Some(name.to_string());
        self
    }

    /// Set the description (help text)
    pub fn description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Define as a boolean type with default value
    pub fn bool_type(mut self, default: bool) -> Self {
        self.ty = Some(OptionType::Bool);
        self.default = Some(OptionValue::Bool(default));
        self
    }

    /// Define as an unsigned integer type with default and optional constraints
    pub fn uint_type(mut self, default: u64, min: Option<u64>, max: Option<u64>) -> Self {
        self.ty = Some(OptionType::UInt { min, max });
        self.default = Some(OptionValue::UInt(default));
        self
    }

    /// Define as a signed integer type with default and optional constraints
    pub fn int_type(mut self, default: i64, min: Option<i64>, max: Option<i64>) -> Self {
        self.ty = Some(OptionType::Int { min, max });
        self.default = Some(OptionValue::Int(default));
        self
    }

    /// Define as a float type with default and optional constraints
    pub fn float_type(mut self, default: f64, min: Option<f64>, max: Option<f64>) -> Self {
        self.ty = Some(OptionType::Float { min, max });
        self.default = Some(OptionValue::Float(default));
        self
    }

    /// Define as a string type with default and optional max length
    pub fn string_type(mut self, default: &str, max_length: Option<usize>) -> Self {
        self.ty = Some(OptionType::String { max_length });
        self.default = Some(OptionValue::String(default.to_string()));
        self
    }

    /// Define as an enum type with allowed variants and default value
    pub fn enum_type(mut self, variants: Vec<&str>, default: &str) -> Self {
        self.ty = Some(OptionType::Enum {
            variants: variants.iter().map(|s| s.to_string()).collect(),
        });
        self.default = Some(OptionValue::String(default.to_string()));
        self
    }

    /// Define as a keybind type with a KeyBinding default
    ///
    /// Keybinds are stored as strings in the database but this method
    /// accepts KeyBinding or KeyCode types for convenience.
    pub fn keybind_type(mut self, default: impl Into<crate::tui::KeyBinding>) -> Self {
        let keybind: crate::tui::KeyBinding = default.into();
        self.ty = Some(OptionType::String {
            max_length: Some(32),
        });
        self.default = Some(OptionValue::String(keybind.to_string()));
        self
    }

    /// Build the option definition
    ///
    /// Returns an error if required fields are missing
    pub fn build(self) -> Result<OptionDefinition> {
        let display_name = self
            .display_name
            .ok_or_else(|| anyhow::anyhow!("display_name is required"))?;
        let ty = self.ty.ok_or_else(|| {
            anyhow::anyhow!("type is required (use bool_type, uint_type, enum_type, etc.)")
        })?;
        let default = self
            .default
            .ok_or_else(|| anyhow::anyhow!("default value is required"))?;

        let key = format!("{}.{}", self.namespace, self.local_key);

        Ok(OptionDefinition {
            key,
            namespace: self.namespace,
            local_key: self.local_key,
            display_name,
            description: self.description.unwrap_or_default(),
            ty,
            default,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_builder() {
        let def = OptionDefBuilder::new("test", "my_bool")
            .display_name("My Bool")
            .description("A test boolean")
            .bool_type(true)
            .build()
            .unwrap();

        assert_eq!(def.key, "test.my_bool");
        assert_eq!(def.namespace, "test");
        assert_eq!(def.local_key, "my_bool");
        assert_eq!(def.display_name, "My Bool");
        assert_eq!(def.description, "A test boolean");
        assert!(matches!(def.ty, OptionType::Bool));
        assert_eq!(def.default, OptionValue::Bool(true));
    }

    #[test]
    fn test_uint_builder_with_constraints() {
        let def = OptionDefBuilder::new("api", "retry.max_attempts")
            .display_name("Max Retry Attempts")
            .description("Maximum number of retries")
            .uint_type(3, Some(1), Some(10))
            .build()
            .unwrap();

        assert_eq!(def.key, "api.retry.max_attempts");
        assert_eq!(def.default, OptionValue::UInt(3));

        match def.ty {
            OptionType::UInt { min, max } => {
                assert_eq!(min, Some(1));
                assert_eq!(max, Some(10));
            }
            _ => panic!("Expected UInt type"),
        }
    }

    #[test]
    fn test_enum_builder() {
        let def = OptionDefBuilder::new("tui", "focus_mode")
            .display_name("Focus Mode")
            .description("How elements gain focus")
            .enum_type(vec!["click", "hover", "hybrid"], "hover")
            .build()
            .unwrap();

        assert_eq!(def.key, "tui.focus_mode");
        assert_eq!(def.default, OptionValue::String("hover".to_string()));

        match def.ty {
            OptionType::Enum { variants } => {
                assert_eq!(variants.len(), 3);
                assert!(variants.contains(&"click".to_string()));
                assert!(variants.contains(&"hover".to_string()));
                assert!(variants.contains(&"hybrid".to_string()));
            }
            _ => panic!("Expected Enum type"),
        }
    }

    #[test]
    fn test_missing_display_name() {
        let result = OptionDefBuilder::new("test", "option")
            .bool_type(true)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_missing_type() {
        let result = OptionDefBuilder::new("test", "option")
            .display_name("Test")
            .build();

        assert!(result.is_err());
    }
}
