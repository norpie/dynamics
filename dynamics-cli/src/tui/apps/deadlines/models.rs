use std::collections::{HashMap, HashSet};
use chrono::NaiveDate;

/// Parameters passed from FileSelectApp to MappingApp
#[derive(Clone, Debug)]
pub struct MappingParams {
    pub file_path: std::path::PathBuf,
    pub sheet_name: String,
    /// If true, this import is specifically for Board of Directors meetings (NRQ only)
    pub board_of_directors_import: bool,
}

// ============================================================================
// Existing Deadline Types (for edit/update support)
// ============================================================================

/// An existing deadline record fetched from Dynamics 365
#[derive(Clone, Debug)]
pub struct ExistingDeadline {
    /// The GUID of the existing record
    pub id: String,
    /// The deadline name (cgk_deadlinename or nrq_deadlinename)
    pub name: String,
    /// The deadline date (date portion only, for matching)
    pub date: NaiveDate,
    /// All field values from the record (for diffing)
    pub fields: HashMap<String, serde_json::Value>,
    /// Existing N:N associations
    pub associations: ExistingAssociations,
}

/// Existing N:N associations for a deadline
#[derive(Clone, Debug, Default)]
pub struct ExistingAssociations {
    /// Support IDs (CGK: N:N, NRQ: via custom junction)
    pub support_ids: HashSet<String>,
    /// Support ID → name map
    pub support_names: HashMap<String, String>,
    /// Category IDs
    pub category_ids: HashSet<String>,
    /// Category ID → name map
    pub category_names: HashMap<String, String>,
    /// Length IDs (CGK only)
    pub length_ids: HashSet<String>,
    /// Length ID → name map
    pub length_names: HashMap<String, String>,
    /// Flemish share IDs
    pub flemishshare_ids: HashSet<String>,
    /// Flemish share ID → name map
    pub flemishshare_names: HashMap<String, String>,
    /// Subcategory IDs (NRQ only)
    pub subcategory_ids: HashSet<String>,
    /// Subcategory ID → name map
    pub subcategory_names: HashMap<String, String>,
    /// Custom junction records for NRQ deadlinesupport (includes extra fields)
    pub custom_junction_records: Vec<ExistingJunctionRecord>,
}

/// An existing custom junction record (e.g., nrq_deadlinesupport)
#[derive(Clone, Debug)]
pub struct ExistingJunctionRecord {
    /// The junction record's own GUID (for deletion)
    pub junction_id: String,
    /// The related entity's GUID (e.g., support ID)
    pub related_id: String,
    /// The related entity's name
    pub related_name: String,
}

/// Lookup key for matching deadlines: (name, date, description)
pub type DeadlineLookupKey = (String, NaiveDate, Option<String>);

/// Lookup map from (name, date, description) → ExistingDeadline
pub type DeadlineLookupMap = HashMap<DeadlineLookupKey, ExistingDeadline>;

// ============================================================================
// Deadline Mode (for edit/update support)
// ============================================================================

/// The mode/action for a transformed deadline record
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeadlineMode {
    /// No match found → create new record
    Create,
    /// Match found, changes detected → update existing record
    Update,
    /// Match found, no changes → show in UI but skip operations
    Unchanged,
    /// Error state (e.g., multiple matches found)
    Error(String),
}

impl Default for DeadlineMode {
    fn default() -> Self {
        DeadlineMode::Create
    }
}

impl std::fmt::Display for DeadlineMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeadlineMode::Create => write!(f, "Create"),
            DeadlineMode::Update => write!(f, "Update"),
            DeadlineMode::Unchanged => write!(f, "Unchanged"),
            DeadlineMode::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl Default for MappingParams {
    fn default() -> Self {
        Self {
            file_path: std::path::PathBuf::new(),
            sheet_name: String::new(),
            board_of_directors_import: false,
        }
    }
}

/// Record for a custom junction entity (e.g., nrq_deadlinesupport)
#[derive(Clone, Debug)]
pub struct CustomJunctionRecord {
    /// The junction entity name (e.g., "nrq_deadlinesupport")
    pub junction_entity: String,
    /// The lookup field on the junction that points to the main entity (e.g., "nrq_deadlineid")
    pub main_entity_field: String,
    /// The lookup field on the junction that points to the related entity (e.g., "nrq_supportid")
    pub related_entity_field: String,
    /// The related entity name (e.g., "nrq_support")
    pub related_entity: String,
    /// The GUID of the related record
    pub related_id: String,
    /// The name/label used to find the related record (e.g., the Excel column header)
    pub related_name: String,
}

/// A single transformed record ready for API creation or update
#[derive(Clone, Debug)]
pub struct TransformedDeadline {
    /// Excel row number (for error reporting)
    pub source_row: usize,

    /// Direct field values (cgk_name/nrq_name, cgk_info/nrq_info, etc.)
    pub direct_fields: std::collections::HashMap<String, String>,

    /// Resolved lookup field IDs (field_name -> (GUID, target_entity))
    pub lookup_fields: std::collections::HashMap<String, (String, String)>,

    /// Resolved checkbox IDs for N:N relationships
    /// Key = relationship name (e.g., "cgk_deadline_cgk_support")
    /// Value = Vec of GUIDs for checked items
    pub checkbox_relationships: std::collections::HashMap<String, Vec<String>>,

    /// Custom junction records for relationships that use separate entities
    /// (e.g., nrq_deadlinesupport instead of a simple N:N)
    pub custom_junction_records: Vec<CustomJunctionRecord>,

    /// Picklist field values (field_name -> integer value)
    pub picklist_fields: std::collections::HashMap<String, i32>,

    /// Boolean field values (field_name -> bool)
    pub boolean_fields: std::collections::HashMap<String, bool>,

    /// Parsed deadline date (cgk_date or nrq_date)
    pub deadline_date: Option<chrono::NaiveDate>,

    /// Parsed deadline time - combined with deadline_date
    pub deadline_time: Option<chrono::NaiveTime>,

    /// Parsed commission date (cgk_datumcommissievergadering or nrq_committeemeetingdate)
    pub commission_date: Option<chrono::NaiveDate>,

    /// Parsed commission time - combined with commission_date
    pub commission_time: Option<chrono::NaiveTime>,

    /// OPM column notes (if any)
    pub notes: Option<String>,

    /// Warnings for this specific row (unresolved lookups, validation errors)
    pub warnings: Vec<String>,

    // ========================================================================
    // Edit mode fields (for update support)
    // ========================================================================

    /// The mode/action for this record (Create/Update/Unchanged/Error)
    pub mode: DeadlineMode,

    /// If matched to an existing record, this is the GUID
    pub existing_guid: Option<String>,

    /// If matched, the existing record's field values (for diffing)
    pub existing_fields: Option<HashMap<String, serde_json::Value>>,

    /// If matched, the existing record's N:N associations (for diffing)
    pub existing_associations: Option<ExistingAssociations>,
}

impl TransformedDeadline {
    pub fn new(source_row: usize) -> Self {
        Self {
            source_row,
            direct_fields: std::collections::HashMap::new(),
            lookup_fields: std::collections::HashMap::new(),
            checkbox_relationships: std::collections::HashMap::new(),
            custom_junction_records: Vec::new(),
            picklist_fields: std::collections::HashMap::new(),
            boolean_fields: std::collections::HashMap::new(),
            deadline_date: None,
            deadline_time: None,
            commission_date: None,
            commission_time: None,
            notes: None,
            warnings: Vec::new(),
            // Edit mode fields - default to Create
            mode: DeadlineMode::Create,
            existing_guid: None,
            existing_fields: None,
            existing_associations: None,
        }
    }

    /// Get the deadline name from direct_fields (cgk_deadlinename or nrq_deadlinename)
    pub fn get_deadline_name(&self, entity_type: &str) -> Option<&str> {
        let name_field = if entity_type == "cgk_deadline" {
            "cgk_deadlinename"
        } else {
            "nrq_deadlinename"
        };
        self.direct_fields.get(name_field).map(|s| s.as_str())
    }

    /// Check if this record is in Create mode
    pub fn is_create(&self) -> bool {
        matches!(self.mode, DeadlineMode::Create)
    }

    /// Check if this record is in Update mode
    pub fn is_update(&self) -> bool {
        matches!(self.mode, DeadlineMode::Update)
    }

    /// Check if this record is Unchanged
    pub fn is_unchanged(&self) -> bool {
        matches!(self.mode, DeadlineMode::Unchanged)
    }

    /// Check if this record has an error
    pub fn is_error(&self) -> bool {
        matches!(self.mode, DeadlineMode::Error(_))
    }

    /// Check if this record has any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Parameters passed from MappingApp to InspectionApp
#[derive(Clone, Debug)]
pub struct InspectionParams {
    pub entity_type: String, // "cgk_deadline" or "nrq_deadline"
    pub transformed_records: Vec<TransformedDeadline>,
}

impl Default for InspectionParams {
    fn default() -> Self {
        Self {
            entity_type: String::new(),
            transformed_records: Vec::new(),
        }
    }
}
