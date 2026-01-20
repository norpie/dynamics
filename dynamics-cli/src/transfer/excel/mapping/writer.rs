//! Write TransferConfig to Excel format

use anyhow::{Context, Result};
use rust_xlsxwriter::{Workbook, Worksheet};

use crate::transfer::{EntityMapping, FieldMapping, TransferConfig, Transform};

use super::values::{
    condition_value, fallback_default_value, format_condition_op, format_fallback, format_value,
};

/// Column indices for mapping Excel
mod cols {
    pub const SOURCE_ENTITY: u16 = 0;
    pub const TARGET_ENTITY: u16 = 1;
    pub const PRIORITY: u16 = 2;
    pub const TARGET_FIELD: u16 = 3;
    pub const TRANSFORM_TYPE: u16 = 4;
    pub const SOURCE_FIELD: u16 = 5;
    pub const CONDITION: u16 = 6;
    pub const CONDITION_VALUE: u16 = 7;
    pub const THEN_VALUE: u16 = 8;
    pub const ELSE_VALUE: u16 = 9;
    pub const FALLBACK: u16 = 10;
    pub const DEFAULT_VALUE: u16 = 11;
}

/// Write a TransferConfig to an Excel file
pub fn write_mapping_excel(config: &TransferConfig, path: &str) -> Result<()> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    worksheet.set_name("Mappings")?;
    write_header(worksheet)?;

    let mut row: u32 = 1;
    for entity in &config.entity_mappings {
        row = write_entity_mapping(worksheet, row, entity)?;
    }

    workbook
        .save(path)
        .with_context(|| format!("Failed to save Excel file: {}", path))?;

    Ok(())
}

fn write_header(ws: &mut Worksheet) -> Result<()> {
    ws.write_string(0, cols::SOURCE_ENTITY, "source_entity")?;
    ws.write_string(0, cols::TARGET_ENTITY, "target_entity")?;
    ws.write_string(0, cols::PRIORITY, "priority")?;
    ws.write_string(0, cols::TARGET_FIELD, "target_field")?;
    ws.write_string(0, cols::TRANSFORM_TYPE, "transform_type")?;
    ws.write_string(0, cols::SOURCE_FIELD, "source_field")?;
    ws.write_string(0, cols::CONDITION, "condition")?;
    ws.write_string(0, cols::CONDITION_VALUE, "condition_value")?;
    ws.write_string(0, cols::THEN_VALUE, "then_value")?;
    ws.write_string(0, cols::ELSE_VALUE, "else_value")?;
    ws.write_string(0, cols::FALLBACK, "fallback")?;
    ws.write_string(0, cols::DEFAULT_VALUE, "default_value")?;
    Ok(())
}

fn write_entity_mapping(ws: &mut Worksheet, start_row: u32, entity: &EntityMapping) -> Result<u32> {
    let mut row = start_row;

    for field in &entity.field_mappings {
        row = write_field_mapping(ws, row, entity, field)?;
    }

    Ok(row)
}

fn write_field_mapping(
    ws: &mut Worksheet,
    start_row: u32,
    entity: &EntityMapping,
    field: &FieldMapping,
) -> Result<u32> {
    match &field.transform {
        Transform::Copy {
            source_path,
            resolver,
        } => {
            write_common_cols(ws, start_row, entity, &field.target_field)?;
            if resolver.is_some() {
                ws.write_string(start_row, cols::TRANSFORM_TYPE, "copy_resolved")?;
                ws.write_string(start_row, cols::FALLBACK, resolver.as_deref().unwrap_or(""))?;
            } else {
                ws.write_string(start_row, cols::TRANSFORM_TYPE, "copy")?;
            }
            ws.write_string(start_row, cols::SOURCE_FIELD, &source_path.to_string())?;
            Ok(start_row + 1)
        }

        Transform::Constant { value } => {
            write_common_cols(ws, start_row, entity, &field.target_field)?;
            ws.write_string(start_row, cols::TRANSFORM_TYPE, "constant")?;
            ws.write_string(start_row, cols::THEN_VALUE, &format_value(value))?;
            Ok(start_row + 1)
        }

        Transform::Conditional {
            source_path,
            condition,
            then_value,
            else_value,
        } => {
            write_common_cols(ws, start_row, entity, &field.target_field)?;
            ws.write_string(start_row, cols::TRANSFORM_TYPE, "conditional")?;
            ws.write_string(start_row, cols::SOURCE_FIELD, &source_path.to_string())?;
            ws.write_string(start_row, cols::CONDITION, format_condition_op(condition))?;
            if let Some(cond_val) = condition_value(condition) {
                ws.write_string(start_row, cols::CONDITION_VALUE, &format_value(cond_val))?;
            }
            ws.write_string(start_row, cols::THEN_VALUE, &format_value(then_value))?;
            ws.write_string(start_row, cols::ELSE_VALUE, &format_value(else_value))?;
            Ok(start_row + 1)
        }

        Transform::ValueMap {
            source_path,
            mappings,
            fallback,
        } => {
            // Write header row for value_map
            write_common_cols(ws, start_row, entity, &field.target_field)?;
            ws.write_string(start_row, cols::TRANSFORM_TYPE, "value_map")?;
            ws.write_string(start_row, cols::SOURCE_FIELD, &source_path.to_string())?;
            ws.write_string(start_row, cols::FALLBACK, format_fallback(fallback))?;
            if let Some(default_val) = fallback_default_value(fallback) {
                ws.write_string(start_row, cols::DEFAULT_VALUE, &format_value(default_val))?;
            }

            let mut row = start_row + 1;

            // Write entry rows
            for (from_val, to_val) in mappings {
                write_common_cols(ws, row, entity, &field.target_field)?;
                ws.write_string(row, cols::TRANSFORM_TYPE, "value_map_entry")?;
                ws.write_string(row, cols::CONDITION_VALUE, &format_value(from_val))?;
                ws.write_string(row, cols::THEN_VALUE, &format_value(to_val))?;
                row += 1;
            }

            Ok(row)
        }

        Transform::Format {
            template,
            null_handling,
        } => {
            write_common_cols(ws, start_row, entity, &field.target_field)?;
            ws.write_string(start_row, cols::TRANSFORM_TYPE, "format")?;
            // Template goes in then_value column
            ws.write_string(start_row, cols::THEN_VALUE, &template.to_string())?;
            // Null handling goes in fallback column (reusing existing column)
            let null_str = match null_handling {
                crate::transfer::transform::format::NullHandling::Error => "error",
                crate::transfer::transform::format::NullHandling::Empty => "empty",
                crate::transfer::transform::format::NullHandling::Zero => "zero",
            };
            ws.write_string(start_row, cols::FALLBACK, null_str)?;
            Ok(start_row + 1)
        }

        Transform::Replace {
            source_path,
            replacements,
        } => {
            // Write header row for replace
            write_common_cols(ws, start_row, entity, &field.target_field)?;
            ws.write_string(start_row, cols::TRANSFORM_TYPE, "replace")?;
            ws.write_string(start_row, cols::SOURCE_FIELD, &source_path.to_string())?;

            let mut row = start_row + 1;

            // Write entry rows for each replacement
            for r in replacements {
                write_common_cols(ws, row, entity, &field.target_field)?;
                ws.write_string(row, cols::TRANSFORM_TYPE, "replace_entry")?;
                ws.write_string(row, cols::CONDITION_VALUE, &r.pattern)?;
                ws.write_string(row, cols::THEN_VALUE, &r.replacement)?;
                // Use FALLBACK column for is_regex flag
                ws.write_string(row, cols::FALLBACK, if r.is_regex { "regex" } else { "" })?;
                row += 1;
            }

            Ok(row)
        }
    }
}

fn write_common_cols(
    ws: &mut Worksheet,
    row: u32,
    entity: &EntityMapping,
    target_field: &str,
) -> Result<()> {
    ws.write_string(row, cols::SOURCE_ENTITY, &entity.source_entity)?;
    ws.write_string(row, cols::TARGET_ENTITY, &entity.target_entity)?;
    ws.write_number(row, cols::PRIORITY, entity.priority as f64)?;
    ws.write_string(row, cols::TARGET_FIELD, target_field)?;
    Ok(())
}
