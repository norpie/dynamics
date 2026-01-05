//! Lookup binding types for proper @odata.bind format generation

use std::collections::HashMap;

use crate::api::metadata::{FieldMetadata, FieldType};

/// Info needed to bind a lookup field to the OData API
#[derive(Debug, Clone)]
pub struct LookupBindingInfo {
    /// Logical field name (e.g., "parentcustomerid")
    pub field_name: String,
    /// Schema name with proper casing (e.g., "ParentCustomerId")
    pub schema_name: String,
    /// Target entity set name (e.g., "accounts")
    pub target_entity_set: String,
}

/// Context for transforming lookup fields to @odata.bind format
#[derive(Debug, Clone, Default)]
pub struct LookupBindingContext {
    /// Map: field_name -> binding info
    pub lookups: HashMap<String, LookupBindingInfo>,
}

/// Error building lookup binding context
#[derive(Debug, Clone)]
pub enum LookupBindingError {
    /// Field is a polymorphic lookup (has multiple target entities)
    PolymorphicLookup {
        field_name: String,
        targets: Vec<String>,
    },
    /// Missing schema name for lookup field
    MissingSchemaName { field_name: String },
    /// Missing entity set for target entity
    MissingEntitySet { entity_name: String },
}

impl std::fmt::Display for LookupBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LookupBindingError::PolymorphicLookup { field_name, targets } => {
                write!(
                    f,
                    "Polymorphic lookup '{}' has multiple targets ({}) - use a single-target field instead",
                    field_name,
                    targets.join(", ")
                )
            }
            LookupBindingError::MissingSchemaName { field_name } => {
                write!(
                    f,
                    "Missing schema name for lookup field '{}' - metadata may be incomplete",
                    field_name
                )
            }
            LookupBindingError::MissingEntitySet { entity_name } => {
                write!(
                    f,
                    "Missing entity set name for entity '{}' - cannot build @odata.bind",
                    entity_name
                )
            }
        }
    }
}

impl std::error::Error for LookupBindingError {}

impl LookupBindingContext {
    /// Build lookup binding context from field metadata
    ///
    /// # Arguments
    /// * `fields` - Field metadata for the target entity
    /// * `entity_set_map` - Map from entity logical name to entity set name
    ///
    /// # Errors
    /// Returns error if:
    /// - A lookup field has multiple targets (polymorphic)
    /// - A lookup field is missing schema_name
    /// - Target entity is not in entity_set_map
    pub fn from_field_metadata(
        fields: &[FieldMetadata],
        entity_set_map: &HashMap<String, String>,
    ) -> Result<Self, LookupBindingError> {
        let mut lookups = HashMap::new();

        for field in fields {
            // Only process lookup fields
            if !matches!(field.field_type, FieldType::Lookup) {
                continue;
            }

            // Get target entity - must be single target
            let target_entity = match &field.related_entity {
                Some(target) => target,
                None => continue, // No target means not a real lookup (skip)
            };

            // Check for schema name
            let schema_name = field.schema_name.as_ref().ok_or_else(|| {
                LookupBindingError::MissingSchemaName {
                    field_name: field.logical_name.clone(),
                }
            })?;

            // Get entity set name for target
            let target_entity_set =
                entity_set_map
                    .get(target_entity)
                    .ok_or_else(|| LookupBindingError::MissingEntitySet {
                        entity_name: target_entity.clone(),
                    })?;

            lookups.insert(
                field.logical_name.clone(),
                LookupBindingInfo {
                    field_name: field.logical_name.clone(),
                    schema_name: schema_name.clone(),
                    target_entity_set: target_entity_set.clone(),
                },
            );
        }

        Ok(LookupBindingContext { lookups })
    }

    /// Check if a field is a lookup that needs binding
    pub fn is_lookup(&self, field_name: &str) -> bool {
        self.lookups.contains_key(field_name)
    }

    /// Get binding info for a field
    pub fn get(&self, field_name: &str) -> Option<&LookupBindingInfo> {
        self.lookups.get(field_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lookup_field(name: &str, schema: &str, target: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            schema_name: Some(schema.to_string()),
            display_name: None,
            field_type: FieldType::Lookup,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: Some(target.to_string()),
            navigation_property_name: None,
            option_values: vec![],
        }
    }

    fn make_string_field(name: &str) -> FieldMetadata {
        FieldMetadata {
            logical_name: name.to_string(),
            schema_name: Some(name.to_string()),
            display_name: None,
            field_type: FieldType::String,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: None,
            navigation_property_name: None,
            option_values: vec![],
        }
    }

    #[test]
    fn test_build_context_basic() {
        let fields = vec![
            make_lookup_field("parentaccountid", "ParentAccountId", "account"),
            make_lookup_field("primarycontactid", "PrimaryContactId", "contact"),
            make_string_field("name"),
        ];

        let mut entity_set_map = HashMap::new();
        entity_set_map.insert("account".to_string(), "accounts".to_string());
        entity_set_map.insert("contact".to_string(), "contacts".to_string());

        let ctx = LookupBindingContext::from_field_metadata(&fields, &entity_set_map).unwrap();

        assert_eq!(ctx.lookups.len(), 2);

        let parent = ctx.get("parentaccountid").unwrap();
        assert_eq!(parent.schema_name, "ParentAccountId");
        assert_eq!(parent.target_entity_set, "accounts");

        let contact = ctx.get("primarycontactid").unwrap();
        assert_eq!(contact.schema_name, "PrimaryContactId");
        assert_eq!(contact.target_entity_set, "contacts");

        assert!(!ctx.is_lookup("name"));
    }

    #[test]
    fn test_missing_schema_name_errors() {
        let fields = vec![FieldMetadata {
            logical_name: "badlookup".to_string(),
            schema_name: None, // Missing!
            display_name: None,
            field_type: FieldType::Lookup,
            is_required: false,
            is_primary_key: false,
            max_length: None,
            related_entity: Some("account".to_string()),
            navigation_property_name: None,
            option_values: vec![],
        }];

        let mut entity_set_map = HashMap::new();
        entity_set_map.insert("account".to_string(), "accounts".to_string());

        let result = LookupBindingContext::from_field_metadata(&fields, &entity_set_map);
        assert!(matches!(
            result,
            Err(LookupBindingError::MissingSchemaName { .. })
        ));
    }

    #[test]
    fn test_missing_entity_set_errors() {
        let fields = vec![make_lookup_field(
            "parentaccountid",
            "ParentAccountId",
            "account",
        )];

        let entity_set_map = HashMap::new(); // Empty - no entity sets

        let result = LookupBindingContext::from_field_metadata(&fields, &entity_set_map);
        assert!(matches!(
            result,
            Err(LookupBindingError::MissingEntitySet { .. })
        ));
    }
}
