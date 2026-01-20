//! Global registry for option definitions

use super::types::OptionDefinition;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::RwLock;

/// Thread-safe global registry of option definitions
pub struct OptionsRegistry {
    definitions: RwLock<HashMap<String, OptionDefinition>>,
}

impl OptionsRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            definitions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new option definition
    ///
    /// Returns an error if an option with the same key is already registered
    pub fn register(&self, def: OptionDefinition) -> Result<()> {
        let mut defs = self.definitions.write().unwrap();
        if defs.contains_key(&def.key) {
            anyhow::bail!("Option '{}' is already registered", def.key);
        }
        log::debug!("Registered option: {} ({})", def.key, def.display_name);
        defs.insert(def.key.clone(), def);
        Ok(())
    }

    /// Get option definition by full key
    pub fn get(&self, key: &str) -> Option<OptionDefinition> {
        self.definitions.read().unwrap().get(key).cloned()
    }

    /// Check if an option is registered
    pub fn contains(&self, key: &str) -> bool {
        self.definitions.read().unwrap().contains_key(key)
    }

    /// List all options in a namespace
    pub fn list_namespace(&self, namespace: &str) -> Vec<OptionDefinition> {
        let defs = self.definitions.read().unwrap();
        let mut options: Vec<_> = defs
            .values()
            .filter(|def| def.namespace == namespace)
            .cloned()
            .collect();

        // Sort by key for consistent ordering
        options.sort_by(|a, b| a.key.cmp(&b.key));
        options
    }

    /// Get all unique namespaces
    pub fn namespaces(&self) -> Vec<String> {
        let defs = self.definitions.read().unwrap();
        let mut namespaces: Vec<_> = defs.values().map(|def| def.namespace.clone()).collect();

        namespaces.sort();
        namespaces.dedup();
        namespaces
    }

    /// Get total number of registered options
    pub fn count(&self) -> usize {
        self.definitions.read().unwrap().len()
    }

    /// List all option definitions
    pub fn list_all(&self) -> Vec<OptionDefinition> {
        let defs = self.definitions.read().unwrap();
        let mut options: Vec<_> = defs.values().cloned().collect();
        options.sort_by(|a, b| a.key.cmp(&b.key));
        options
    }
}

impl Default for OptionsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::options::types::{OptionType, OptionValue};

    #[test]
    fn test_register_and_get() {
        let registry = OptionsRegistry::new();

        let def = OptionDefinition {
            key: "test.option".to_string(),
            namespace: "test".to_string(),
            local_key: "option".to_string(),
            display_name: "Test Option".to_string(),
            description: "A test option".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(true),
        };

        registry.register(def.clone()).unwrap();

        let retrieved = registry.get("test.option").unwrap();
        assert_eq!(retrieved.key, "test.option");
        assert_eq!(retrieved.display_name, "Test Option");
    }

    #[test]
    fn test_duplicate_registration() {
        let registry = OptionsRegistry::new();

        let def = OptionDefinition {
            key: "test.option".to_string(),
            namespace: "test".to_string(),
            local_key: "option".to_string(),
            display_name: "Test Option".to_string(),
            description: "A test option".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(true),
        };

        registry.register(def.clone()).unwrap();
        let result = registry.register(def);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_namespace() {
        let registry = OptionsRegistry::new();

        let def1 = OptionDefinition {
            key: "api.option1".to_string(),
            namespace: "api".to_string(),
            local_key: "option1".to_string(),
            display_name: "Option 1".to_string(),
            description: "".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(true),
        };

        let def2 = OptionDefinition {
            key: "api.option2".to_string(),
            namespace: "api".to_string(),
            local_key: "option2".to_string(),
            display_name: "Option 2".to_string(),
            description: "".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(false),
        };

        let def3 = OptionDefinition {
            key: "tui.option3".to_string(),
            namespace: "tui".to_string(),
            local_key: "option3".to_string(),
            display_name: "Option 3".to_string(),
            description: "".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(true),
        };

        registry.register(def1).unwrap();
        registry.register(def2).unwrap();
        registry.register(def3).unwrap();

        let api_options = registry.list_namespace("api");
        assert_eq!(api_options.len(), 2);

        let tui_options = registry.list_namespace("tui");
        assert_eq!(tui_options.len(), 1);
    }

    #[test]
    fn test_namespaces() {
        let registry = OptionsRegistry::new();

        let def1 = OptionDefinition {
            key: "api.option".to_string(),
            namespace: "api".to_string(),
            local_key: "option".to_string(),
            display_name: "API Option".to_string(),
            description: "".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(true),
        };

        let def2 = OptionDefinition {
            key: "tui.option".to_string(),
            namespace: "tui".to_string(),
            local_key: "option".to_string(),
            display_name: "TUI Option".to_string(),
            description: "".to_string(),
            ty: OptionType::Bool,
            default: OptionValue::Bool(true),
        };

        registry.register(def1).unwrap();
        registry.register(def2).unwrap();

        let namespaces = registry.namespaces();
        assert_eq!(namespaces.len(), 2);
        assert!(namespaces.contains(&"api".to_string()));
        assert!(namespaces.contains(&"tui".to_string()));
    }
}
