/// Parameters passed from FileSelectApp to MappingApp
#[derive(Clone, Debug)]
pub struct MappingParams {
    pub file_path: std::path::PathBuf,
    pub sheet_name: String,
}

impl Default for MappingParams {
    fn default() -> Self {
        Self {
            file_path: std::path::PathBuf::new(),
            sheet_name: String::new(),
        }
    }
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

    /// Picklist field values (field_name -> integer value)
    pub picklist_fields: std::collections::HashMap<String, i32>,

    /// Parsed deadline date (cgk_date or nrq_date)
    pub deadline_date: Option<chrono::NaiveDate>,

    /// Parsed deadline time - combined with deadline_date
    pub deadline_time: Option<chrono::NaiveTime>,

    /// Parsed commission date (cgk_datumcommissievergadering - CGK only)
    pub commission_date: Option<chrono::NaiveDate>,

    /// Parsed commission time - combined with commission_date (CGK only)
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
            picklist_fields: std::collections::HashMap::new(),
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
