//! Write ResolvedEntity to Excel format

use anyhow::{Context, Result};
use rust_xlsxwriter::{Workbook, Worksheet};

use crate::transfer::{RecordAction, ResolvedEntity, Value};

/// Special column names (prefixed with _)
mod special_cols {
    pub const ACTION: &str = "_action";
    pub const SOURCE_ID: &str = "_source_id";
    pub const ERROR: &str = "_error";
}

/// Write a ResolvedEntity to an Excel file
pub fn write_resolved_excel(entity: &ResolvedEntity, path: &str) -> Result<()> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    worksheet.set_name(&entity.entity_name)?;

    // Build column list: special columns + field columns
    let mut columns: Vec<&str> = vec![
        special_cols::ACTION,
        special_cols::SOURCE_ID,
    ];

    // Add field columns
    for field_name in &entity.field_names {
        columns.push(field_name);
    }

    // Add error column last
    columns.push(special_cols::ERROR);

    // Write header
    for (col, name) in columns.iter().enumerate() {
        worksheet.write_string(0, col as u16, *name)?;
    }

    // Write records
    for (row_idx, record) in entity.records.iter().enumerate() {
        let row = (row_idx + 1) as u32;

        // _action
        worksheet.write_string(row, 0, format_action(&record.action))?;

        // _source_id
        worksheet.write_string(row, 1, &record.source_id.to_string())?;

        // Field values
        for (col_idx, field_name) in entity.field_names.iter().enumerate() {
            let col = (col_idx + 2) as u16; // +2 for _action and _source_id
            if let Some(value) = record.fields.get(field_name) {
                write_value(worksheet, row, col, value)?;
            }
        }

        // _error (last column)
        let error_col = (columns.len() - 1) as u16;
        if let Some(ref error) = record.error {
            worksheet.write_string(row, error_col, error)?;
        }
    }

    workbook.save(path).with_context(|| format!("Failed to save Excel file: {}", path))?;

    Ok(())
}

fn format_action(action: &RecordAction) -> &'static str {
    match action {
        RecordAction::Create => "create",
        RecordAction::Update => "update",
        RecordAction::Delete => "delete",
        RecordAction::Deactivate => "deactivate",
        RecordAction::NoChange => "nochange",
        RecordAction::TargetOnly => "target-only",
        RecordAction::Skip => "skip",
        RecordAction::Error => "error",
    }
}

fn write_value(ws: &mut Worksheet, row: u32, col: u16, value: &Value) -> Result<()> {
    match value {
        Value::Null => { /* Leave cell empty */ }
        Value::String(s) => { ws.write_string(row, col, s)?; }
        Value::Int(i) => { ws.write_number(row, col, *i as f64)?; }
        Value::Float(f) => { ws.write_number(row, col, *f)?; }
        Value::Bool(b) => { ws.write_string(row, col, &b.to_string())?; }
        Value::DateTime(dt) => { ws.write_string(row, col, &dt.to_rfc3339())?; }
        Value::Guid(g) => { ws.write_string(row, col, &g.to_string())?; }
        Value::OptionSet(i) => { ws.write_number(row, col, *i as f64)?; }
        Value::Dynamic(d) => { ws.write_string(row, col, &d.to_string())?; }
    }
    Ok(())
}
