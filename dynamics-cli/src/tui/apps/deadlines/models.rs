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
    /// Category IDs
    pub category_ids: HashSet<String>,
    /// Length IDs (CGK only)
    pub length_ids: HashSet<String>,
    /// Flemish share IDs
    pub flemishshare_ids: HashSet<String>,
    /// Subcategory IDs (NRQ only)
    pub subcategory_ids: HashSet<String>,
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

/// Lookup key for matching deadlines: (name, date)
pub type DeadlineLookupKey = (String, NaiveDate);

/// Lookup map from (name, date) → ExistingDeadline
pub type DeadlineLookupMap = HashMap<DeadlineLookupKey, ExistingDeadline>;

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

/// A single transformed record ready for API creation
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
        }
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
