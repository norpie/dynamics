//! Read TransferConfig from Excel format

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use calamine::{Data, Reader, Xlsx, open_workbook};

use crate::transfer::{
    Condition, EntityMapping, Fallback, FieldMapping, FieldPath, Replacement, TransferConfig,
    Transform, Value,
};

use super::values::{parse_condition, parse_fallback, parse_value};

/// Column indices (must match writer)
mod cols {
    pub const SOURCE_ENTITY: usize = 0;
    pub const TARGET_ENTITY: usize = 1;
    pub const PRIORITY: usize = 2;
    pub const TARGET_FIELD: usize = 3;
    pub const TRANSFORM_TYPE: usize = 4;
    pub const SOURCE_FIELD: usize = 5;
    pub const CONDITION: usize = 6;
    pub const CONDITION_VALUE: usize = 7;
    pub const THEN_VALUE: usize = 8;
    pub const ELSE_VALUE: usize = 9;
    pub const FALLBACK: usize = 10;
    pub const DEFAULT_VALUE: usize = 11;
}

/// Read a TransferConfig from an Excel file
pub fn read_mapping_excel(
    path: &str,
    config_name: &str,
    source_env: &str,
    target_env: &str,
) -> Result<TransferConfig> {
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

    let mut config = TransferConfig::new(config_name, source_env, target_env);

    // Track entity mappings by (source, target) key
    let mut entities: HashMap<(String, String), EntityMapping> = HashMap::new();

    // Track pending value_map to attach entries to
    let mut pending_value_map: Option<PendingValueMap> = None;

    // Track pending replace to attach entries to
    let mut pending_replace: Option<PendingReplace> = None;

    // Skip header row
    for (row_idx, row) in range.rows().enumerate().skip(1) {
        let row_num = row_idx + 1; // 1-based for error messages

        // Skip empty rows
        if row.iter().all(|c| c.to_string().trim().is_empty()) {
            continue;
        }

        let source_entity = get_cell_string(row, cols::SOURCE_ENTITY);
        let target_entity = get_cell_string(row, cols::TARGET_ENTITY);
        let priority = get_cell_int(row, cols::PRIORITY).unwrap_or(1) as u32;
        let target_field = get_cell_string(row, cols::TARGET_FIELD);
        let transform_type = get_cell_string(row, cols::TRANSFORM_TYPE).to_lowercase();

        if source_entity.is_empty() || target_entity.is_empty() {
            continue; // Skip rows without entity info
        }

        let entity_key = (source_entity.clone(), target_entity.clone());

        // Handle value_map_entry specially
        if transform_type == "value_map_entry" {
            if let Some(ref mut pending) = pending_value_map {
                let from_val = parse_value(&get_cell_string(row, cols::CONDITION_VALUE));
                let to_val = parse_value(&get_cell_string(row, cols::THEN_VALUE));
                pending.entries.push((from_val, to_val));
            } else {
                bail!(
                    "Row {}: value_map_entry without preceding value_map",
                    row_num
                );
            }
            continue;
        }

        // Handle replace_entry specially
        if transform_type == "replace_entry" {
            if let Some(ref mut pending) = pending_replace {
                let pattern = get_cell_string(row, cols::CONDITION_VALUE);
                let replacement = get_cell_string(row, cols::THEN_VALUE);
                let is_regex = get_cell_string(row, cols::FALLBACK).to_lowercase() == "regex";
                pending
                    .entries
                    .push(Replacement::new(pattern, replacement, is_regex));
            } else {
                bail!("Row {}: replace_entry without preceding replace", row_num);
            }
            continue;
        }

        // If we have a pending value_map and hit a non-entry row, finalize it
        if let Some(pending) = pending_value_map.take() {
            finalize_value_map(&mut entities, pending)?;
        }

        // If we have a pending replace and hit a non-entry row, finalize it
        if let Some(pending) = pending_replace.take() {
            finalize_replace(&mut entities, pending)?;
        }

        // Parse the transform
        let transform = match transform_type.as_str() {
            "copy" | "copy_resolved" => {
                let source_field = get_cell_string(row, cols::SOURCE_FIELD);
                if source_field.is_empty() {
                    bail!("Row {}: copy transform requires source_field", row_num);
                }
                let resolver = if transform_type == "copy_resolved" {
                    let resolver_name = get_cell_string(row, cols::FALLBACK);
                    if resolver_name.is_empty() {
                        None
                    } else {
                        Some(resolver_name)
                    }
                } else {
                    None
                };
                Transform::Copy {
                    source_path: FieldPath::parse(&source_field)
                        .with_context(|| format!("Row {}: invalid source_field path", row_num))?,
                    resolver,
                }
            }

            "constant" => {
                let value = parse_value(&get_cell_string(row, cols::THEN_VALUE));
                Transform::Constant { value }
            }

            "conditional" => {
                let source_field = get_cell_string(row, cols::SOURCE_FIELD);
                let condition_op = get_cell_string(row, cols::CONDITION);
                let condition_val = get_cell_string(row, cols::CONDITION_VALUE);
                let then_val = parse_value(&get_cell_string(row, cols::THEN_VALUE));
                let else_val = parse_value(&get_cell_string(row, cols::ELSE_VALUE));

                if source_field.is_empty() {
                    bail!(
                        "Row {}: conditional transform requires source_field",
                        row_num
                    );
                }

                let condition =
                    parse_condition(&condition_op, &condition_val).with_context(|| {
                        format!("Row {}: invalid condition '{}'", row_num, condition_op)
                    })?;

                Transform::Conditional {
                    source_path: FieldPath::parse(&source_field)
                        .with_context(|| format!("Row {}: invalid source_field path", row_num))?,
                    condition,
                    then_value: then_val,
                    else_value: else_val,
                }
            }

            "value_map" => {
                let source_field = get_cell_string(row, cols::SOURCE_FIELD);
                let fallback_type = get_cell_string(row, cols::FALLBACK);
                let default_val = get_cell_string(row, cols::DEFAULT_VALUE);

                if source_field.is_empty() {
                    bail!("Row {}: value_map transform requires source_field", row_num);
                }

                // Start collecting entries
                pending_value_map = Some(PendingValueMap {
                    entity_key: entity_key.clone(),
                    priority,
                    target_field: target_field.clone(),
                    source_path: FieldPath::parse(&source_field)
                        .with_context(|| format!("Row {}: invalid source_field path", row_num))?,
                    fallback: parse_fallback(&fallback_type, &default_val),
                    entries: Vec::new(),
                });
                continue; // Don't add field yet, wait for entries
            }

            "format" => {
                let template_str = get_cell_string(row, cols::THEN_VALUE);
                let null_handling_str = get_cell_string(row, cols::FALLBACK);

                if template_str.is_empty() {
                    bail!(
                        "Row {}: format transform requires template (in then_value column)",
                        row_num
                    );
                }

                let template = crate::transfer::transform::format::parse_template(&template_str)
                    .map_err(|e| {
                        anyhow::anyhow!("Row {}: invalid format template: {}", row_num, e)
                    })?;

                let null_handling = match null_handling_str.to_lowercase().as_str() {
                    "empty" => crate::transfer::transform::format::NullHandling::Empty,
                    "zero" => crate::transfer::transform::format::NullHandling::Zero,
                    _ => crate::transfer::transform::format::NullHandling::Error, // default
                };

                Transform::Format {
                    template,
                    null_handling,
                }
            }

            "replace" => {
                let source_field = get_cell_string(row, cols::SOURCE_FIELD);

                if source_field.is_empty() {
                    bail!("Row {}: replace transform requires source_field", row_num);
                }

                // Start collecting entries
                pending_replace = Some(PendingReplace {
                    entity_key: entity_key.clone(),
                    priority,
                    target_field: target_field.clone(),
                    source_path: FieldPath::parse(&source_field)
                        .with_context(|| format!("Row {}: invalid source_field path", row_num))?,
                    entries: Vec::new(),
                });
                continue; // Don't add field yet, wait for entries
            }

            other => {
                bail!("Row {}: unknown transform_type '{}'", row_num, other);
            }
        };

        // Get or create entity mapping
        let entity = entities
            .entry(entity_key.clone())
            .or_insert_with(|| EntityMapping::new(&source_entity, &target_entity, priority));

        entity.add_field_mapping(FieldMapping::new(&target_field, transform));
    }

    // Finalize any remaining pending value_map
    if let Some(pending) = pending_value_map.take() {
        finalize_value_map(&mut entities, pending)?;
    }

    // Finalize any remaining pending replace
    if let Some(pending) = pending_replace.take() {
        finalize_replace(&mut entities, pending)?;
    }

    // Add entities to config in priority order
    let mut entity_list: Vec<_> = entities.into_values().collect();
    entity_list.sort_by_key(|e| e.priority);
    for entity in entity_list {
        config.add_entity_mapping(entity);
    }

    Ok(config)
}

/// Pending value_map being collected
struct PendingValueMap {
    entity_key: (String, String),
    priority: u32,
    target_field: String,
    source_path: FieldPath,
    fallback: Fallback,
    entries: Vec<(Value, Value)>,
}

/// Pending replace being collected
struct PendingReplace {
    entity_key: (String, String),
    priority: u32,
    target_field: String,
    source_path: FieldPath,
    entries: Vec<Replacement>,
}

fn finalize_value_map(
    entities: &mut HashMap<(String, String), EntityMapping>,
    pending: PendingValueMap,
) -> Result<()> {
    let transform = Transform::ValueMap {
        source_path: pending.source_path,
        mappings: pending.entries,
        fallback: pending.fallback,
    };

    let entity = entities
        .entry(pending.entity_key.clone())
        .or_insert_with(|| {
            EntityMapping::new(
                &pending.entity_key.0,
                &pending.entity_key.1,
                pending.priority,
            )
        });

    entity.add_field_mapping(FieldMapping::new(&pending.target_field, transform));
    Ok(())
}

fn finalize_replace(
    entities: &mut HashMap<(String, String), EntityMapping>,
    pending: PendingReplace,
) -> Result<()> {
    let transform = Transform::Replace {
        source_path: pending.source_path,
        replacements: pending.entries,
    };

    let entity = entities
        .entry(pending.entity_key.clone())
        .or_insert_with(|| {
            EntityMapping::new(
                &pending.entity_key.0,
                &pending.entity_key.1,
                pending.priority,
            )
        });

    entity.add_field_mapping(FieldMapping::new(&pending.target_field, transform));
    Ok(())
}

fn get_cell_string(row: &[Data], col: usize) -> String {
    row.get(col)
        .map(|c| match c {
            Data::String(s) => s.clone(),
            Data::Int(i) => i.to_string(),
            Data::Float(f) => {
                // Check if it's a whole number
                if f.fract() == 0.0 {
                    (*f as i64).to_string()
                } else {
                    f.to_string()
                }
            }
            Data::Bool(b) => b.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn get_cell_int(row: &[Data], col: usize) -> Option<i64> {
    row.get(col).and_then(|c| match c {
        Data::Int(i) => Some(*i),
        Data::Float(f) => Some(*f as i64),
        Data::String(s) => s.parse().ok(),
        _ => None,
    })
}
