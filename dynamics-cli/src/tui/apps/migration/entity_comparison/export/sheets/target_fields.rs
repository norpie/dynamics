//! Target Fields sheet - field mapping details from target perspective

use anyhow::Result;
use rust_xlsxwriter::*;
use std::collections::HashMap;

use crate::api::metadata::FieldMetadata;
use crate::tui::Resource;
use super::super::super::app::State;
use super::super::super::{MatchInfo, MatchType};
use super::super::formatting::*;

/// Write a field row with the new column structure (no Required/Primary Key)
fn write_field_row(
    sheet: &mut Worksheet,
    row: u32,
    field: &FieldMetadata,
    mapped_from: &str,
    mapped_type: &str,
    match_type: &str,
    row_format: &Format,
    indent_format: &Format,
) -> Result<()> {
    // Columns: Field Name | Field Type | Match Type | Related Entity | Mapped From | Mapped Type
    sheet.write_string_with_format(row, 0, &format!("    {}", field.logical_name), indent_format)?;
    sheet.write_string_with_format(row, 1, &format!("{:?}", field.field_type), row_format)?;
    sheet.write_string_with_format(row, 2, match_type, row_format)?;

    // Related Entity (only for Lookup fields)
    let related_entity_str = field.related_entity.as_deref().unwrap_or("");
    sheet.write_string_with_format(row, 3, related_entity_str, row_format)?;

    sheet.write_string_with_format(row, 4, mapped_from, row_format)?;
    sheet.write_string_with_format(row, 5, mapped_type, row_format)?;

    Ok(())
}

/// Create target fields sheet with mapping information (reverse perspective)
pub fn create_target_fields_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Target Fields")?;

    let header_format = create_header_format();
    let title_format = create_title_format();

    // Title
    let target_entities_str = if state.target_entities.len() > 1 {
        format!("{} entities: {} (showing first only)", state.target_entities.len(), state.target_entities.join(", "))
    } else {
        state.target_entities.first().map(|s| s.as_str()).unwrap_or("").to_string()
    };
    sheet.write_string_with_format(
        0,
        0,
        &format!("Target Fields - {} ({})", target_entities_str, state.target_env),
        &title_format,
    )?;

    // Headers
    let headers = ["Field Name", "Field Type", "Match Type", "Related Entity", "Mapped From", "Mapped Type"];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_string_with_format(2, col as u16, *header, &header_format)?;
    }

    let mut row = 3u32;
    let exact_match_format = create_exact_match_format();
    let manual_mapping_format = create_manual_mapping_format();
    let prefix_match_format = create_prefix_match_format();
    let type_mismatch_format = create_type_mismatch_format();
    let example_value_format = create_example_value_format();
    let unmapped_format = create_unmapped_format();
    let indent_format = Format::new().set_indent(1);

    // Get target fields
    // TODO: Support multi-entity mode - for now use first entity
    let target_fields = if let Some(first_entity) = state.target_entities.first() {
        match state.target_metadata.get(first_entity) {
            Some(Resource::Success(metadata)) => &metadata.fields,
            _ => {
                sheet.write_string(row, 0, "No metadata loaded")?;
                sheet.autofit();
                return Ok(());
            }
        }
    } else {
        sheet.write_string(row, 0, "No target entities")?;
        sheet.autofit();
        return Ok(());
    };

    // Get source fields for type lookup
    let source_fields = if let Some(first_entity) = state.source_entities.first() {
        match state.source_metadata.get(first_entity) {
            Some(Resource::Success(metadata)) => &metadata.fields,
            _ => {
                sheet.write_string(row, 0, "No source metadata loaded")?;
                sheet.autofit();
                return Ok(());
            }
        }
    } else {
        sheet.write_string(row, 0, "No source entities")?;
        sheet.autofit();
        return Ok(());
    };

    // Create a lookup map for source field types
    let source_field_types: HashMap<_, _> = source_fields
        .iter()
        .map(|f| (f.logical_name.as_str(), format!("{:?}", f.field_type)))
        .collect();

    // Reverse lookup: find source fields that map to each target field
    // For 1-to-N mappings, a target could have multiple sources mapping to it
    let mut reverse_matches: HashMap<String, Vec<(String, MatchType)>> = HashMap::new();
    for (source_field, match_info) in &state.field_matches {
        // For each target in this match_info, add a reverse mapping
        for target_field in &match_info.target_fields {
            let match_type = match_info.match_types.get(target_field).cloned().unwrap_or(MatchType::Manual);

            reverse_matches.entry(target_field.clone())
                .or_insert_with(Vec::new)
                .push((source_field.clone(), match_type));
        }
    }

    // Partition fields into mapped, unmapped, and ignored
    let mut mapped_fields = Vec::new();
    let mut unmapped_fields = Vec::new();
    let mut ignored_fields = Vec::new();

    for field in target_fields {
        let ignore_id = format!("fields:target:{}", field.logical_name);

        if state.ignored_items.contains(&ignore_id) {
            ignored_fields.push(field);
        } else if reverse_matches.contains_key(&field.logical_name) {
            mapped_fields.push(field);
        } else {
            unmapped_fields.push(field);
        }
    }

    // MAPPED FIELDS Section
    if !mapped_fields.is_empty() {
        sheet.write_string_with_format(row, 0, "✓ MAPPED FIELDS", &header_format)?;
        row += 1;

        // Group by match type (using first source's match type)
        let exact_matches: Vec<_> = mapped_fields.iter().filter(|f| {
            reverse_matches.get(&f.logical_name)
                .and_then(|sources| sources.first())
                .map(|(_, mt)| mt == &MatchType::Exact)
                .unwrap_or(false)
        }).collect();

        let manual_mappings: Vec<_> = mapped_fields.iter().filter(|f| {
            reverse_matches.get(&f.logical_name)
                .and_then(|sources| sources.first())
                .map(|(_, mt)| mt == &MatchType::Manual)
                .unwrap_or(false)
        }).collect();

        let prefix_matches: Vec<_> = mapped_fields.iter().filter(|f| {
            reverse_matches.get(&f.logical_name)
                .and_then(|sources| sources.first())
                .map(|(_, mt)| mt == &MatchType::Prefix)
                .unwrap_or(false)
        }).collect();

        let type_mismatches: Vec<_> = mapped_fields.iter().filter(|f| {
            reverse_matches.get(&f.logical_name)
                .and_then(|sources| sources.first())
                .map(|(_, mt)| matches!(mt, MatchType::TypeMismatch(_)))
                .unwrap_or(false)
        }).collect();

        let example_matches: Vec<_> = mapped_fields.iter().filter(|f| {
            reverse_matches.get(&f.logical_name)
                .and_then(|sources| sources.first())
                .map(|(_, mt)| mt == &MatchType::ExampleValue)
                .unwrap_or(false)
        }).collect();

        let import_matches: Vec<_> = mapped_fields.iter().filter(|f| {
            reverse_matches.get(&f.logical_name)
                .and_then(|sources| sources.first())
                .map(|(_, mt)| mt == &MatchType::Import)
                .unwrap_or(false)
        }).collect();

        // Exact Matches
        if !exact_matches.is_empty() {
            sheet.write_string_with_format(row, 0, "  Exact Name + Type Matches", &Format::new().set_bold())?;
            row += 1;
            for field in exact_matches {
                if let Some(sources) = reverse_matches.get(&field.logical_name) {
                    let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                    let source_names_str = source_names.join(", ");
                    let source_types_str = source_names
                        .iter()
                        .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &source_names_str, &source_types_str, "Exact", &exact_match_format, &indent_format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Manual Mappings
        if !manual_mappings.is_empty() {
            sheet.write_string_with_format(row, 0, "  Manual Mappings", &Format::new().set_bold())?;
            row += 1;
            for field in manual_mappings {
                if let Some(sources) = reverse_matches.get(&field.logical_name) {
                    let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                    let source_names_str = source_names.join(", ");
                    let source_types_str = source_names
                        .iter()
                        .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &source_names_str, &source_types_str, "Manual", &manual_mapping_format, &indent_format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Prefix Matches
        if !prefix_matches.is_empty() {
            sheet.write_string_with_format(row, 0, "  Prefix Matches", &Format::new().set_bold())?;
            row += 1;
            for field in prefix_matches {
                if let Some(sources) = reverse_matches.get(&field.logical_name) {
                    let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                    let source_names_str = source_names.join(", ");
                    let source_types_str = source_names
                        .iter()
                        .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &source_names_str, &source_types_str, "Prefix", &prefix_match_format, &indent_format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Type Mismatches
        if !type_mismatches.is_empty() {
            sheet.write_string_with_format(row, 0, "  Type Mismatches", &Format::new().set_bold().set_font_color(Color::RGB(0xFF8C00)))?;
            row += 1;
            for field in type_mismatches {
                if let Some(sources) = reverse_matches.get(&field.logical_name) {
                    let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                    let source_names_str = source_names.join(", ");
                    let source_types_str = source_names
                        .iter()
                        .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &source_names_str, &source_types_str, "Type Mismatch", &type_mismatch_format, &indent_format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Example Value Matches
        if !example_matches.is_empty() {
            sheet.write_string_with_format(row, 0, "  Example Value Matches", &Format::new().set_bold())?;
            row += 1;
            for field in example_matches {
                if let Some(sources) = reverse_matches.get(&field.logical_name) {
                    let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                    let source_names_str = source_names.join(", ");
                    let source_types_str = source_names
                        .iter()
                        .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &source_names_str, &source_types_str, "Example", &example_value_format, &indent_format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Import Matches
        if !import_matches.is_empty() {
            sheet.write_string_with_format(row, 0, "  Imported Mappings", &Format::new().set_bold())?;
            row += 1;
            for field in import_matches {
                if let Some(sources) = reverse_matches.get(&field.logical_name) {
                    let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                    let source_names_str = source_names.join(", ");
                    let source_types_str = source_names
                        .iter()
                        .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &source_names_str, &source_types_str, "Import", &manual_mapping_format, &indent_format)?;
                    row += 1;
                }
            }
            row += 1;
        }
    }

    // UNMAPPED FIELDS Section
    if !unmapped_fields.is_empty() {
        sheet.write_string_with_format(row, 0, "⚠ UNMAPPED FIELDS", &header_format)?;
        row += 1;

        for field in unmapped_fields {
            write_field_row(sheet, row, field, "", "", "Unmapped", &unmapped_format, &indent_format)?;
            row += 1;
        }
        row += 1;
    }

    // IGNORED FIELDS Section
    if !ignored_fields.is_empty() {
        sheet.write_string_with_format(row, 0, "🚫 IGNORED FIELDS", &header_format)?;
        row += 1;

        for field in ignored_fields {
            // Check if it has a reverse mapping (ignored but mapped)
            let (mapped_from, mapped_type, match_type) = if let Some(sources) = reverse_matches.get(&field.logical_name) {
                let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                let source_names_str = source_names.join(", ");
                let source_types_str = source_names
                    .iter()
                    .map(|sn| source_field_types.get(sn).map(|s| s.as_str()).unwrap_or("Unknown"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let match_type_str = sources.first()
                    .map(|(_, mt)| format!("{:?}", mt))
                    .unwrap_or_else(|| "Unknown".to_string());
                (source_names_str, source_types_str, match_type_str)
            } else {
                (String::new(), String::new(), "Ignored".to_string())
            };

            write_field_row(sheet, row, field, &mapped_from, &mapped_type, &match_type, &unmapped_format, &indent_format)?;
            row += 1;
        }
    }

    sheet.autofit();
    Ok(())
}
