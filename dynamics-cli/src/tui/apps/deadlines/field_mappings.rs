/// Static field mappings for deadline Excel imports
///
/// This module defines the mapping from Excel column names to Dynamics fields
/// for both cgk_deadlines and nrq_deadlines entities.
///
/// Checkbox columns (after "Raad van Bestuur") are NOT defined here - they are
/// dynamically discovered from entity metadata.

use std::collections::HashMap;

/// Entities that need to be fetched and cached for lookups and checkboxes
/// These are used for both field lookups and N:N relationships
pub const LOOKUP_ENTITIES_CGK: &[&str] = &[
    "cgk_pillar",       // Lookup: Pillar/Domain
    "cgk_fund",         // Lookup: Fund
    "cgk_commission",   // Lookup: Commission
    "systemuser",       // Lookup: Project manager (no prefix)
    "cgk_support",      // Checkbox: Support
    "cgk_category",     // Checkbox: Category
    "cgk_length",       // Checkbox: Subcategory (called length in CGK)
    "cgk_flemishshare", // Checkbox: Flemish share
    "cgk_deadline",     // Lookup: Board meeting (self-referencing)
];

pub const LOOKUP_ENTITIES_NRQ: &[&str] = &[
    "nrq_domain",                  // Lookup: Pillar/Domain (called domain in NRQ)
    "nrq_fund",                    // Lookup: Fund
    "nrq_commission",              // Lookup: Commission
    "systemuser",                  // Lookup: Project manager (no prefix)
    "nrq_support",                 // Checkbox: Support
    "nrq_category",                // Checkbox: Category
    "nrq_subcategory",             // Checkbox: Subcategory
    "nrq_flemishshare",            // Checkbox: Flemish share
    "nrq_boardofdirectorsmeeting", // Lookup: Board meeting (separate entity)
    "nrq_type",                    // Lookup: Type (NRQ only)
];

/// Get the list of entities to cache based on detected deadline entity type
pub fn get_cache_entities(entity_type: &str) -> Vec<String> {
    let entities = match entity_type {
        "cgk_deadline" => LOOKUP_ENTITIES_CGK,
        "nrq_deadline" => LOOKUP_ENTITIES_NRQ,
        _ => return Vec::new(),
    };

    entities.iter().map(|s| s.to_string()).collect()
}

/// Field type for mapping configuration
#[derive(Debug, Clone)]
pub enum FieldType {
    /// Direct field - simple value copy
    Direct,
    /// Lookup field - requires entity resolution via CSV cache
    Lookup { target_entity: String },
    /// Date field - requires parsing and timezone conversion
    Date,
    /// Time field - combined with date field for datetime
    Time,
    /// Checkbox field - N:N relationship (dynamically discovered)
    Checkbox,
    /// Picklist field - maps label to integer value
    Picklist { options: HashMap<String, i32> },
    /// Boolean field - maps string values to true/false
    Boolean { true_value: String, false_value: String },
    /// Custom junction entity - for relationships that use a separate entity instead of N:N
    /// Used when the junction has additional fields (e.g., nrq_deadlinesupport)
    CustomJunction {
        /// The junction entity name (e.g., "nrq_deadlinesupport")
        junction_entity: String,
        /// The lookup field on the junction that points to the main entity (e.g., "nrq_deadlineid")
        main_entity_field: String,
        /// The lookup field on the junction that points to the related entity (e.g., "nrq_supportid")
        related_entity_field: String,
        /// The related entity name for lookups (e.g., "nrq_support")
        related_entity: String,
    },
}

/// Single field mapping from Excel column to Dynamics field
#[derive(Debug, Clone)]
pub struct FieldMapping {
    pub excel_column: String,
    pub dynamics_field: String,
    pub field_type: FieldType,
    pub required: bool,
}

/// Get field mappings for cgk_deadlines entity
pub fn get_cgk_mappings() -> Vec<FieldMapping> {
    vec![
        // Pillar (prefer "Pillar" over "Domein*" - Pillar is machine-readable)
        FieldMapping {
            excel_column: "Pillar".to_string(),
            dynamics_field: "cgk_pillarid".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "cgk_pillar".to_string(),
            },
            required: false,
        },

        // Fund
        FieldMapping {
            excel_column: "Fonds*".to_string(),
            dynamics_field: "cgk_fundid".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "cgk_fund".to_string(),
            },
            required: false,
        },

        // Deadline name (just the value from Deadline column)
        FieldMapping {
            excel_column: "Deadline*".to_string(),
            dynamics_field: "cgk_deadlinename".to_string(),
            field_type: FieldType::Direct,
            required: false,
        },

        // Project manager
        FieldMapping {
            excel_column: "Projectbeheerder".to_string(),
            dynamics_field: "cgk_projectmanagerid".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "systemuser".to_string(),
            },
            required: false,
        },

        // Info field
        FieldMapping {
            excel_column: "Info".to_string(),
            dynamics_field: "cgk_info".to_string(),
            field_type: FieldType::Direct,
            required: false,
        },

        // Deadline date (combined with time from "Uur" column)
        FieldMapping {
            excel_column: "Datum Deadline".to_string(),
            dynamics_field: "cgk_date".to_string(),
            field_type: FieldType::Date,
            required: false,
        },

        // Time for deadline (combined into cgk_date)
        FieldMapping {
            excel_column: "Uur".to_string(),
            dynamics_field: "cgk_date".to_string(),
            field_type: FieldType::Time,
            required: false,
        },

        // Commission lookup
        FieldMapping {
            excel_column: "Commissie".to_string(),
            dynamics_field: "cgk_commissionid".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "cgk_commission".to_string(),
            },
            required: false,
        },

        // Commission meeting date
        FieldMapping {
            excel_column: "Datum Commissievergadering".to_string(),
            dynamics_field: "cgk_datumcommissievergadering".to_string(),
            field_type: FieldType::Date,
            required: false,
        },

        // Commission meeting time (combined into cgk_datumcommissievergadering)
        FieldMapping {
            excel_column: "Uur Commissie".to_string(),
            dynamics_field: "cgk_datumcommissievergadering".to_string(),
            field_type: FieldType::Time,
            required: false,
        },

        // Board meeting (Raad van Bestuur) - hidden field only visible in $metadata
        FieldMapping {
            excel_column: "Raad van Bestuur".to_string(),
            dynamics_field: "cgk_raadvanbestuur_cgk_deadline".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "cgk_deadline".to_string(), // Self-referencing for CGK
            },
            required: false,
        },

        // Support Type - picklist field
        FieldMapping {
            excel_column: "Support Type".to_string(),
            dynamics_field: "vaf_support_type".to_string(),
            field_type: FieldType::Picklist {
                options: {
                    let mut map = HashMap::new();
                    map.insert("Automatische steun".to_string(), 806150000);
                    map.insert("Selectieve steun".to_string(), 806150001);
                    map
                },
            },
            required: true,
        },
    ]
}

/// Get field mappings for nrq_deadlines entity
pub fn get_nrq_mappings() -> Vec<FieldMapping> {
    vec![
        // Domain - try "Domein*" first, fallback to "Pillar"
        FieldMapping {
            excel_column: "Domein*".to_string(),
            dynamics_field: "nrq_DomainId".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "nrq_domain".to_string(),
            },
            required: false,
        },
        // Domain fallback - "Pillar" if "Domein*" not found
        FieldMapping {
            excel_column: "Pillar".to_string(),
            dynamics_field: "nrq_DomainId".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "nrq_domain".to_string(),
            },
            required: false,
        },

        // Fund
        FieldMapping {
            excel_column: "Fonds*".to_string(),
            dynamics_field: "nrq_FundId".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "nrq_fund".to_string(),
            },
            required: false,
        },

        // Deadline name (raw value goes to nrq_deadlinename, nrq_name is generated with date/time)
        FieldMapping {
            excel_column: "Deadline*".to_string(),
            dynamics_field: "nrq_deadlinename".to_string(),
            field_type: FieldType::Direct,
            required: false,
        },

        // Project manager
        FieldMapping {
            excel_column: "Projectbeheerder".to_string(),
            dynamics_field: "nrq_ProjectManagerId".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "systemuser".to_string(),
            },
            required: false,
        },

        // Info field
        FieldMapping {
            excel_column: "Info".to_string(),
            dynamics_field: "nrq_description".to_string(),
            field_type: FieldType::Direct,
            required: false,
        },

        // Deadline date
        FieldMapping {
            excel_column: "Datum Deadline".to_string(),
            dynamics_field: "nrq_deadlinedate".to_string(),
            field_type: FieldType::Date,
            required: false,
        },

        // Deadline time (combined into nrq_deadlinedate)
        FieldMapping {
            excel_column: "Uur".to_string(),
            dynamics_field: "nrq_deadlinedate".to_string(),
            field_type: FieldType::Time,
            required: false,
        },

        // Commission
        FieldMapping {
            excel_column: "Commissie".to_string(),
            dynamics_field: "nrq_CommissionId".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "nrq_commission".to_string(),
            },
            required: false,
        },

        // Board meeting (Raad van Bestuur) - separate entity for NRQ
        FieldMapping {
            excel_column: "Raad van Bestuur".to_string(),
            dynamics_field: "nrq_BoardOfDirectorsMeetingId".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "nrq_boardofdirectorsmeeting".to_string(),
            },
            required: false,
        },

        // Type - lookup to nrq_type (NRQ only, not supported in CGK)
        FieldMapping {
            excel_column: "Type".to_string(),
            dynamics_field: "nrq_TypeID".to_string(),
            field_type: FieldType::Lookup {
                target_entity: "nrq_type".to_string(),
            },
            required: false,
        },

        // Commission meeting date (NRQ has this too, unlike what was previously assumed)
        FieldMapping {
            excel_column: "Datum Commissievergadering".to_string(),
            dynamics_field: "nrq_committeemeetingdate".to_string(),
            field_type: FieldType::Date,
            required: false,
        },

        // Commission meeting time (combined into nrq_committeemeetingdate)
        FieldMapping {
            excel_column: "Uur Commissie".to_string(),
            dynamics_field: "nrq_committeemeetingdate".to_string(),
            field_type: FieldType::Time,
            required: false,
        },

        // Commission meeting in person (Online of Fysiek)
        FieldMapping {
            excel_column: "Online of Fysiek".to_string(),
            dynamics_field: "nrq_committeemeetinginperson".to_string(),
            field_type: FieldType::Boolean {
                true_value: "Fysiek".to_string(),
                false_value: "Online".to_string(),
            },
            required: false,
        },

        // Support Type - picklist field (NRQ uses nrq_supporttypeoptionset with different values)
        FieldMapping {
            excel_column: "Support Type".to_string(),
            dynamics_field: "nrq_supporttypeoptionset".to_string(),
            field_type: FieldType::Picklist {
                options: {
                    let mut map = HashMap::new();
                    map.insert("Automatische steun".to_string(), 875810000);
                    map.insert("Automatic Support".to_string(), 875810000);
                    map.insert("Selectieve steun".to_string(), 875810001);
                    map.insert("Selective Support".to_string(), 875810001);
                    map
                },
            },
            required: true,
        },
    ]
}

/// Build a lookup map from Excel column name to FieldMapping
pub fn build_mapping_lookup(mappings: Vec<FieldMapping>) -> HashMap<String, FieldMapping> {
    mappings
        .into_iter()
        .map(|m| (m.excel_column.clone(), m))
        .collect()
}

/// Detect which entity type based on environment entities
/// Returns either "cgk_deadline" or "nrq_deadline" if found, None otherwise
pub fn detect_deadline_entity(entities: &[String]) -> Option<String> {
    if entities.iter().any(|e| e == "cgk_deadline") {
        Some("cgk_deadline".to_string())
    } else if entities.iter().any(|e| e == "nrq_deadline") {
        Some("nrq_deadline".to_string())
    } else {
        None
    }
}

/// Get mappings based on detected entity type
pub fn get_mappings_for_entity(entity_name: &str) -> Vec<FieldMapping> {
    match entity_name {
        "cgk_deadline" => get_cgk_mappings(),
        "nrq_deadline" => get_nrq_mappings(),
        _ => vec![],
    }
}

/// Get constant fields to add to each record based on entity type
/// These fields are automatically added to every operation created
pub fn get_constant_fields(entity_name: &str) -> HashMap<String, serde_json::Value> {
    use serde_json::json;

    match entity_name {
        "cgk_deadline" => {
            let mut fields = HashMap::new();
            fields.insert("cgk_vafvalidated".to_string(), json!(true));
            fields.insert("vaf_applications_opened".to_string(), json!(false));
            fields
        }
        "nrq_deadline" => {
            let mut fields = HashMap::new();
            fields.insert("nrq_vafvalidated".to_string(), json!(true));
            fields
        }
        _ => unreachable!("Unknown entity type: {}", entity_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cgk_entity() {
        let entities = vec!["cgk_pillar".to_string(), "cgk_deadline".to_string()];
        assert_eq!(detect_deadline_entity(&entities), Some("cgk_deadline".to_string()));
    }

    #[test]
    fn test_detect_nrq_entity() {
        let entities = vec!["nrq_domain".to_string(), "nrq_deadline".to_string()];
        assert_eq!(detect_deadline_entity(&entities), Some("nrq_deadline".to_string()));
    }

    #[test]
    fn test_cgk_mappings_count() {
        let mappings = get_cgk_mappings();
        assert_eq!(mappings.len(), 12); // 12 non-checkbox fields (added Uur Commissie + Support Type)
    }

    #[test]
    fn test_nrq_mappings_count() {
        let mappings = get_nrq_mappings();
        assert_eq!(mappings.len(), 15); // 15 non-checkbox fields (includes commission date/time + Type + Online of Fysiek)
    }
}
