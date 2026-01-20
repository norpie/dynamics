//! Dynamics 365 metadata models

use serde::{Deserialize, Serialize};

/// Dynamics 365 field metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMetadata {
    pub logical_name: String,
    /// Schema name with proper casing (e.g., "ParentCustomerId")
    /// Used for @odata.bind annotations
    pub schema_name: Option<String>,
    pub display_name: Option<String>,
    pub field_type: FieldType,
    pub is_required: bool,
    pub is_primary_key: bool,
    pub max_length: Option<i32>,
    pub related_entity: Option<String>, // For lookups
    /// Navigation property name for OData $expand (e.g., "ownerid", "parentcustomerid_account")
    /// This comes from ManyToOneRelationships.ReferencingEntityNavigationPropertyName
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation_property_name: Option<String>,
    /// Option values for OptionSet/MultiSelectOptionSet fields
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub option_values: Vec<OptionSetValue>,
}

/// Field data types in Dynamics 365
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Decimal,
    Boolean,
    DateTime,
    Lookup,
    OptionSet,
    MultiSelectOptionSet,
    Money,
    Memo,
    UniqueIdentifier,
    Other(String),
}

/// Option value for OptionSet/MultiSelectOptionSet fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionSetValue {
    /// The numeric value (e.g., 1, 2, 3)
    pub value: i64,
    /// The display label (e.g., "Active", "Inactive")
    pub label: Option<String>,
}

/// Relationship metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipMetadata {
    pub name: String,
    pub relationship_type: RelationshipType,
    pub related_entity: String,
    pub related_attribute: String,
}

/// Relationship types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationshipType {
    OneToMany,
    ManyToOne,
    ManyToMany,
}

/// View column metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewColumn {
    pub name: String,
    pub width: Option<u32>,
    pub is_primary: bool,
}

/// View metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewMetadata {
    pub id: String,
    pub name: String,
    pub view_type: String,
    pub columns: Vec<ViewColumn>,
}

/// Form field metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub logical_name: String,
    pub label: String,
    pub visible: bool,
    pub required_level: String, // None, ApplicationRequired, SystemRequired
    pub readonly: bool,
    pub row: i32,
    pub column: i32,
}

/// Form section metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSection {
    pub name: String,
    pub label: String,
    pub visible: bool,
    pub columns: i32,
    pub order: i32,
    pub fields: Vec<FormField>,
}

/// Form tab metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormTab {
    pub name: String,
    pub label: String,
    pub visible: bool,
    pub expanded: bool,
    pub order: i32,
    pub sections: Vec<FormSection>,
}

/// Form structure (nested hierarchy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormStructure {
    pub name: String,
    pub entity_name: String,
    pub tabs: Vec<FormTab>,
}

/// Form metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormMetadata {
    pub id: String,
    pub name: String,
    pub form_type: String,
    pub form_structure: Option<FormStructure>,
}

/// Complete entity metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMetadata {
    pub fields: Vec<FieldMetadata>,
    pub relationships: Vec<RelationshipMetadata>,
    pub views: Vec<ViewMetadata>,
    pub forms: Vec<FormMetadata>,
    /// Entity set name for OData URLs (e.g., "accounts", "contacts")
    #[serde(default)]
    pub entity_set_name: Option<String>,
    /// Primary ID attribute name (e.g., "accountid", "businessprocessflowinstanceid")
    #[serde(default)]
    pub primary_id_attribute: Option<String>,
}

impl Default for EntityMetadata {
    fn default() -> Self {
        Self {
            fields: Vec::new(),
            relationships: Vec::new(),
            views: Vec::new(),
            forms: Vec::new(),
            entity_set_name: None,
            primary_id_attribute: None,
        }
    }
}
