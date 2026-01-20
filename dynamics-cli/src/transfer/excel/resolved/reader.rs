//! Read ResolvedEntity edits from Excel format

use std::collections::HashMap;

use anyhow::{Context, Result};
use calamine::{Data, Reader, Xlsx, open_workbook};
use uuid::Uuid;

use crate::transfer::{RecordAction, ResolvedEntity, ResolvedRecord, Value};

/// Edits imported from Excel
#[derive(Debug, Default)]
pub struct ResolvedEdits {
    /// Records that changed (keyed by source_id)
    pub changed_records: HashMap<Uuid, RecordEdit>,
}

/// Edit to a single record
#[derive(Debug)]
pub struct RecordEdit {
    pub source_id: Uuid,
    pub new_action: Option<RecordAction>,
    pub changed_fields: HashMap<String, Value>,
}

/// Read edits from an Excel file and apply them to a ResolvedEntity
pub fn read_resolved_excel(path: &str, entity: &mut ResolvedEntity) -> Result<ResolvedEdits> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("Failed to open Excel file: {}", path))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .context("Excel file has no sheets")?
        .clone();

    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("Failed to read sheet: {}", sheet_name))?;

    let rows: Vec<_> = range.rows().collect();
    if rows.is_empty() {
        return Ok(ResolvedEdits::default());
    }

    // Parse header to get column indices
    let header = &rows[0];
    let col_indices = parse_header(header, &entity.field_names);

    let mut edits = ResolvedEdits::default();

    // Process each data row
    for (row_idx, row) in rows.iter().enumerate().skip(1) {
        // Get source_id
        let source_id_str = col_indices
            .source_id_col
            .and_then(|c| get_cell_string(row, c))
            .unwrap_or_default();

        let source_id = match Uuid::parse_str(&source_id_str) {
            Ok(id) => id,
            Err(_) => continue, // Skip rows without valid source_id
        };

        // Find the original record
        let original = match entity.find_record(source_id) {
            Some(r) => r,
            None => continue, // Skip rows not in original data
        };

        let mut record_edit = RecordEdit {
            source_id,
            new_action: None,
            changed_fields: HashMap::new(),
        };

        // Check action change
        if let Some(action_col) = col_indices.action_col {
            if let Some(action_str) = get_cell_string(row, action_col) {
                let new_action = parse_action(&action_str);
                if new_action != original.action {
                    record_edit.new_action = Some(new_action);
                }
            }
        }

        // Check field changes
        for (field_name, col) in &col_indices.field_cols {
            if let Some(cell_str) = get_cell_string(row, *col) {
                let new_value = parse_cell_value(&cell_str);

                // Compare with original
                let original_value = original
                    .fields
                    .get(field_name)
                    .cloned()
                    .unwrap_or(Value::Null);
                if new_value != original_value {
                    record_edit
                        .changed_fields
                        .insert(field_name.clone(), new_value);
                }
            }
        }

        // Only track if something changed
        if record_edit.new_action.is_some() || !record_edit.changed_fields.is_empty() {
            edits.changed_records.insert(source_id, record_edit);
        }
    }

    // Apply edits to the entity
    apply_edits(entity, &edits);

    Ok(edits)
}

/// Apply edits to a ResolvedEntity
pub fn apply_edits(entity: &mut ResolvedEntity, edits: &ResolvedEdits) {
    for (source_id, edit) in &edits.changed_records {
        if let Some(record) = entity.find_record_mut(*source_id) {
            // Apply action change
            if let Some(new_action) = edit.new_action {
                record.action = new_action;
                // Clear error if action changed from Error
                if new_action != RecordAction::Error {
                    record.error = None;
                }
            }

            // Apply field changes
            for (field, value) in &edit.changed_fields {
                record.fields.insert(field.clone(), value.clone());
            }

            // Mark as dirty
            entity.mark_dirty(*source_id);
        }
    }
}

struct ColumnIndices {
    action_col: Option<usize>,
    source_id_col: Option<usize>,
    field_cols: HashMap<String, usize>,
}

fn parse_header(header: &[Data], expected_fields: &[String]) -> ColumnIndices {
    let mut indices = ColumnIndices {
        action_col: None,
        source_id_col: None,
        field_cols: HashMap::new(),
    };

    for (col, cell) in header.iter().enumerate() {
        let name = match cell {
            Data::String(s) => s.as_str(),
            _ => continue,
        };

        match name {
            "_action" => indices.action_col = Some(col),
            "_source_id" => indices.source_id_col = Some(col),
            "_error" => { /* Ignore error column on read */ }
            field_name => {
                if expected_fields.contains(&field_name.to_string()) {
                    indices.field_cols.insert(field_name.to_string(), col);
                }
            }
        }
    }

    indices
}

fn get_cell_string(row: &[Data], col: usize) -> Option<String> {
    row.get(col).and_then(|c| match c {
        Data::String(s) if !s.is_empty() => Some(s.clone()),
        Data::Int(i) => Some(i.to_string()),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                Some((*f as i64).to_string())
            } else {
                Some(f.to_string())
            }
        }
        Data::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

fn parse_action(s: &str) -> RecordAction {
    match s.trim().to_lowercase().as_str() {
        "create" => RecordAction::Create,
        "update" => RecordAction::Update,
        "nochange" => RecordAction::NoChange,
        "skip" => RecordAction::Skip,
        "error" => RecordAction::Error,
        _ => RecordAction::Create, // Default to Create for unknown/legacy values
    }
}

fn parse_cell_value(s: &str) -> Value {
    let s = s.trim();

    if s.is_empty() {
        return Value::Null;
    }

    // Boolean
    match s.to_lowercase().as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }

    // Integer
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }

    // Float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }

    // GUID
    if let Ok(g) = Uuid::parse_str(s) {
        return Value::Guid(g);
    }

    // DateTime
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Value::DateTime(dt.with_timezone(&chrono::Utc));
    }

    // Default to string
    Value::String(s.to_string())
}
