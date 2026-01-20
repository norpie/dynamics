//! Read operations from Excel files
//!
//! Parses Excel files with sheets named:
//! - "Create (entity)" -> Operation::Create
//! - "Update (entity)" -> Operation::Update
//! - "Delete (entity)" -> Operation::Delete

use anyhow::{Context, Result, bail};
use calamine::{Data, Reader, Xlsx, open_workbook};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::path::Path;

use crate::api::operations::Operation;

/// Result of parsing an operations Excel file
#[derive(Debug, Clone)]
pub struct ParsedOperations {
    /// Operations grouped by sheet
    pub sheets: Vec<SheetOperations>,
    /// Total operation count
    pub total_count: usize,
}

/// Operations from a single sheet
#[derive(Debug, Clone)]
pub struct SheetOperations {
    /// Sheet name
    pub sheet_name: String,
    /// Entity name (e.g., "nrq_capacities")
    pub entity: String,
    /// Operation type
    pub operation_type: OperationType,
    /// The operations
    pub operations: Vec<Operation>,
}

/// Type of operation from sheet name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Create,
    Update,
    Delete,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Create => write!(f, "Create"),
            OperationType::Update => write!(f, "Update"),
            OperationType::Delete => write!(f, "Delete"),
        }
    }
}

/// Parse sheet name to extract operation type and entity
/// Format: "Operation (entity)" e.g., "Update (nrq_capacities)"
fn parse_sheet_name(name: &str) -> Option<(OperationType, String)> {
    let re = Regex::new(r"^(Create|Update|Delete)\s*\(([^)]+)\)$").ok()?;
    let caps = re.captures(name)?;

    let op_type = match caps.get(1)?.as_str() {
        "Create" => OperationType::Create,
        "Update" => OperationType::Update,
        "Delete" => OperationType::Delete,
        _ => return None,
    };

    let entity = caps.get(2)?.as_str().trim().to_string();
    Some((op_type, entity))
}

/// Derive singular entity name from plural (for primary key detection)
/// e.g., "nrq_capacities" -> "nrq_capacity", "contacts" -> "contact"
fn entity_singular(entity: &str) -> String {
    if entity.ends_with("ies") {
        format!("{}y", &entity[..entity.len() - 3])
    } else if entity.ends_with("ses")
        || entity.ends_with("xes")
        || entity.ends_with("ches")
        || entity.ends_with("shes")
    {
        entity[..entity.len() - 2].to_string()
    } else if entity.ends_with('s') {
        entity[..entity.len() - 1].to_string()
    } else {
        entity.to_string()
    }
}

/// Find primary key column index
/// Looks for column ending with "id" that matches entity singular name
fn find_primary_key_col(headers: &[String], entity: &str) -> Option<(usize, String)> {
    let singular = entity_singular(entity);
    let expected_pk = format!("{}id", singular);

    // First try exact match
    for (i, h) in headers.iter().enumerate() {
        if h.to_lowercase() == expected_pk.to_lowercase() {
            return Some((i, h.clone()));
        }
    }

    // Fallback: any column ending with "id" that's not a lookup (@odata.bind)
    for (i, h) in headers.iter().enumerate() {
        if h.to_lowercase().ends_with("id") && !h.contains("@odata.bind") {
            return Some((i, h.clone()));
        }
    }

    None
}

/// Convert Excel cell to serde_json::Value
fn cell_to_value(cell: &Data) -> Value {
    match cell {
        Data::Empty => Value::Null,
        Data::String(s) if s.is_empty() => Value::Null,
        Data::String(s) => {
            // Check for boolean strings
            match s.to_lowercase().as_str() {
                "true" => return Value::Bool(true),
                "false" => return Value::Bool(false),
                _ => {}
            }
            Value::String(s.clone())
        }
        Data::Int(i) => json!(*i),
        Data::Float(f) => {
            // If it's a whole number, use integer
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                json!(*f as i64)
            } else {
                json!(*f)
            }
        }
        Data::Bool(b) => Value::Bool(*b),
        Data::DateTime(dt) => {
            // Excel datetime as string
            Value::String(format!("{}", dt))
        }
        Data::DateTimeIso(s) => Value::String(s.clone()),
        Data::DurationIso(s) => Value::String(s.clone()),
        Data::Error(_) => Value::Null,
    }
}

/// Read operations from an Excel file
pub fn read_operations_excel<P: AsRef<Path>>(path: P) -> Result<ParsedOperations> {
    let path = path.as_ref();
    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Failed to open Excel file: {}", path.display()))?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let mut sheets = Vec::new();
    let mut total_count = 0;

    for sheet_name in sheet_names {
        // Try to parse sheet name
        let (op_type, entity) = match parse_sheet_name(&sheet_name) {
            Some(parsed) => parsed,
            None => continue, // Skip sheets that don't match the pattern
        };

        let range = workbook
            .worksheet_range(&sheet_name)
            .with_context(|| format!("Failed to read sheet: {}", sheet_name))?;

        let rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
        if rows.is_empty() {
            continue;
        }

        // Parse headers (first row)
        let headers: Vec<String> = rows[0]
            .iter()
            .map(|c| match c {
                Data::String(s) => s.clone(),
                _ => String::new(),
            })
            .collect();

        // For Update/Delete, find primary key column
        let pk_info = if op_type == OperationType::Update || op_type == OperationType::Delete {
            let pk = find_primary_key_col(&headers, &entity).with_context(|| {
                format!(
                    "No primary key column found for {} in sheet '{}'",
                    entity, sheet_name
                )
            })?;
            Some(pk)
        } else {
            None
        };

        let mut operations = Vec::new();

        // Process data rows
        for row in rows.iter().skip(1) {
            // Build data object, skipping underscore-prefixed columns
            let mut data = Map::new();
            let mut record_id: Option<String> = None;

            for (col_idx, cell) in row.iter().enumerate() {
                let header = headers.get(col_idx).map(|s| s.as_str()).unwrap_or("");

                // Skip empty headers
                if header.is_empty() {
                    continue;
                }

                // Skip underscore-prefixed metadata columns
                if header.starts_with('_') {
                    continue;
                }

                let value = cell_to_value(cell);

                // Check if this is the primary key column
                if let Some((pk_col, _)) = &pk_info {
                    if col_idx == *pk_col {
                        if let Value::String(s) = &value {
                            record_id = Some(s.clone());
                        }
                        // Don't include PK in data for Update operations
                        if op_type == OperationType::Update {
                            continue;
                        }
                    }
                }

                // Skip null values
                if value.is_null() {
                    continue;
                }

                data.insert(header.to_string(), value);
            }

            // Skip empty rows
            if data.is_empty() && record_id.is_none() {
                continue;
            }

            // Create operation based on type
            let operation = match op_type {
                OperationType::Create => Operation::Create {
                    entity: entity.clone(),
                    data: Value::Object(data),
                },
                OperationType::Update => {
                    let id = record_id.context("Update row missing primary key value")?;
                    Operation::Update {
                        entity: entity.clone(),
                        id,
                        data: Value::Object(data),
                    }
                }
                OperationType::Delete => {
                    let id = record_id.context("Delete row missing primary key value")?;
                    Operation::Delete {
                        entity: entity.clone(),
                        id,
                    }
                }
            };

            operations.push(operation);
        }

        total_count += operations.len();

        sheets.push(SheetOperations {
            sheet_name,
            entity,
            operation_type: op_type,
            operations,
        });
    }

    Ok(ParsedOperations {
        sheets,
        total_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sheet_name() {
        assert_eq!(
            parse_sheet_name("Create (nrq_capacities)"),
            Some((OperationType::Create, "nrq_capacities".to_string()))
        );
        assert_eq!(
            parse_sheet_name("Update (contacts)"),
            Some((OperationType::Update, "contacts".to_string()))
        );
        assert_eq!(
            parse_sheet_name("Delete (accounts)"),
            Some((OperationType::Delete, "accounts".to_string()))
        );
        assert_eq!(parse_sheet_name("Errors"), None);
        assert_eq!(parse_sheet_name("Random Sheet"), None);
    }

    #[test]
    fn test_entity_singular() {
        assert_eq!(entity_singular("nrq_capacities"), "nrq_capacity");
        assert_eq!(entity_singular("contacts"), "contact");
        assert_eq!(entity_singular("accounts"), "account");
        assert_eq!(entity_singular("addresses"), "address");
        assert_eq!(entity_singular("nrq_role"), "nrq_role"); // Already singular
    }
}
