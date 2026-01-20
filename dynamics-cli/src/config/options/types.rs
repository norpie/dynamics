//! Core types for the options system

use anyhow::{Context, Result};

/// A strongly-typed option value
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
}

impl OptionValue {
    /// Get as bool, returning error if wrong type
    pub fn as_bool(&self) -> Result<bool> {
        match self {
            OptionValue::Bool(v) => Ok(*v),
            _ => anyhow::bail!("Expected Bool, got {:?}", self),
        }
    }

    /// Get as uint, returning error if wrong type
    pub fn as_uint(&self) -> Result<u64> {
        match self {
            OptionValue::UInt(v) => Ok(*v),
            _ => anyhow::bail!("Expected UInt, got {:?}", self),
        }
    }

    /// Get as int, returning error if wrong type
    pub fn as_int(&self) -> Result<i64> {
        match self {
            OptionValue::Int(v) => Ok(*v),
            _ => anyhow::bail!("Expected Int, got {:?}", self),
        }
    }

    /// Get as float, returning error if wrong type
    pub fn as_float(&self) -> Result<f64> {
        match self {
            OptionValue::Float(v) => Ok(*v),
            _ => anyhow::bail!("Expected Float, got {:?}", self),
        }
    }

    /// Get as string, returning error if wrong type
    pub fn as_string(&self) -> Result<String> {
        match self {
            OptionValue::String(v) => Ok(v.clone()),
            _ => anyhow::bail!("Expected String, got {:?}", self),
        }
    }
}

/// Type definition with constraints for validation
#[derive(Debug, Clone)]
pub enum OptionType {
    Bool,
    Int { min: Option<i64>, max: Option<i64> },
    UInt { min: Option<u64>, max: Option<u64> },
    Float { min: Option<f64>, max: Option<f64> },
    String { max_length: Option<usize> },
    Enum { variants: Vec<String> },
}

impl OptionType {
    /// Check if a value matches this type
    pub fn matches(&self, value: &OptionValue) -> bool {
        match (self, value) {
            (OptionType::Bool, OptionValue::Bool(_)) => true,
            (OptionType::Int { .. }, OptionValue::Int(_)) => true,
            (OptionType::UInt { .. }, OptionValue::UInt(_)) => true,
            (OptionType::Float { .. }, OptionValue::Float(_)) => true,
            (OptionType::String { .. }, OptionValue::String(_)) => true,
            (OptionType::Enum { .. }, OptionValue::String(_)) => true,
            _ => false,
        }
    }

    /// Validate that a value meets the constraints for this type
    pub fn validate(&self, value: &OptionValue) -> Result<()> {
        if !self.matches(value) {
            anyhow::bail!("Type mismatch: expected {:?}, got {:?}", self, value);
        }

        match (self, value) {
            (OptionType::Int { min, max }, OptionValue::Int(v)) => {
                if let Some(min) = min {
                    if v < min {
                        anyhow::bail!("Value {} is below minimum {}", v, min);
                    }
                }
                if let Some(max) = max {
                    if v > max {
                        anyhow::bail!("Value {} is above maximum {}", v, max);
                    }
                }
                Ok(())
            }
            (OptionType::UInt { min, max }, OptionValue::UInt(v)) => {
                if let Some(min) = min {
                    if v < min {
                        anyhow::bail!("Value {} is below minimum {}", v, min);
                    }
                }
                if let Some(max) = max {
                    if v > max {
                        anyhow::bail!("Value {} is above maximum {}", v, max);
                    }
                }
                Ok(())
            }
            (OptionType::Float { min, max }, OptionValue::Float(v)) => {
                if let Some(min) = min {
                    if v < min {
                        anyhow::bail!("Value {} is below minimum {}", v, min);
                    }
                }
                if let Some(max) = max {
                    if v > max {
                        anyhow::bail!("Value {} is above maximum {}", v, max);
                    }
                }
                Ok(())
            }
            (OptionType::String { max_length }, OptionValue::String(v)) => {
                if let Some(max_length) = max_length {
                    if v.len() > *max_length {
                        anyhow::bail!("String length {} exceeds maximum {}", v.len(), max_length);
                    }
                }
                Ok(())
            }
            (OptionType::Enum { variants }, OptionValue::String(v)) => {
                if !variants.contains(v) {
                    anyhow::bail!(
                        "Value '{}' is not a valid variant. Valid values: {:?}",
                        v,
                        variants
                    );
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Complete definition of an option including metadata for UI generation
#[derive(Debug, Clone)]
pub struct OptionDefinition {
    /// Full key with namespace (e.g., "api.retry.max_attempts")
    pub key: String,

    /// Namespace/category (e.g., "api")
    pub namespace: String,

    /// Local key within namespace (e.g., "retry.max_attempts")
    pub local_key: String,

    /// Human-readable display name (e.g., "Max Retry Attempts")
    pub display_name: String,

    /// Detailed description for help text
    pub description: String,

    /// Type definition with constraints
    pub ty: OptionType,

    /// Default value
    pub default: OptionValue,
}

impl OptionDefinition {
    /// Validate that a value is valid for this option
    pub fn validate(&self, value: &OptionValue) -> Result<()> {
        self.ty.validate(value)
    }
}
