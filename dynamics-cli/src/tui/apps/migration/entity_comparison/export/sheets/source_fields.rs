//! Source Fields sheet - field mapping details from source perspective

use anyhow::Result;
use rust_xlsxwriter::*;

use crate::api::metadata::FieldMetadata;
use crate::tui::Resource;
use super::super::super::app::State;
use super::super::super::MatchType;
use super::super::formatting::*;

/// Write a field row with the new column structure (no Required/Primary Key)
fn write_field_row(
    sheet: &mut Worksheet,
    row: u32,
    field: &FieldMetadata,
    mapped_to: &str,
    mapped_type: &str,
    match_type: &str,
    row_format: &Format,
    indent_format: &Format,
) -> Result<()> {
    // Columns: Field Name | Field Type | Match Type | Related Entity | Mapped To | Mapped Type
    sheet.write_string_with_format(row, 0, &format!("    {}", field.logical_name), indent_format)?;
    sheet.write_string_with_format(row, 1, &format!("{:?}", field.field_type), row_format)?;
    sheet.write_string_with_format(row, 2, match_type, row_format)?;

    // Related Entity (only for Lookup fields)
    let related_entity_str = field.related_entity.as_deref().unwrap_or("");
    sheet.write_string_with_format(row, 3, related_entity_str, row_format)?;

    sheet.write_string_with_format(row, 4, mapped_to, row_format)?;
    sheet.write_string_with_format(row, 5, mapped_type, row_format)?;

    Ok(())
}

/// Create source fields sheet with mapping information
pub fn create_source_fields_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Source Fields")?;

    let header_format = create_header_format();
    let title_format = create_title_format();

    // Title
    let source_entities_str = if state.source_entities.len() > 1 {
        format!("{} entities: {} (showing first only)", state.source_entities.len(), state.source_entities.join(", "))
    } else {
        state.source_entities.first().map(|s| s.as_str()).unwrap_or("").to_string()
    };
    sheet.write_string_with_format(
        0,
        0,
        &format!("Source Fields - {} ({})", source_entities_str, state.source_env),
        &title_format,
    )?;

    // Headers
    let headers = ["Field Name", "Field Type", "Match Type", "Related Entity", "Mapped To", "Mapped Type"];
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

    // Determine if we're in multi-entity mode
    let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

    // Helper closure to compute field key (qualified in multi-entity mode)
    let make_field_key = |entity_name: &str, field_name: &str| -> String {
        if is_multi_entity {
            format!("{}.{}", entity_name, field_name)
        } else {
            field_name.to_string()
        }
    };

    // Get source fields
    // In multi-entity mode, we still export the first entity but use qualified names for lookups
    let first_source_entity = state.source_entities.first();
    let source_fields = if let Some(entity_name) = first_source_entity {
        match state.source_metadata.get(entity_name) {
            Some(Resource::Success(metadata)) => &metadata.fields,
            _ => {
                sheet.write_string(row, 0, "No metadata loaded")?;
                sheet.autofit();
                return Ok(());
            }
        }
    } else {
        sheet.write_string(row, 0, "No source entities")?;
        sheet.autofit();
        return Ok(());
    };

    // Get target fields for type lookup
    let target_fields = if let Some(first_entity) = state.target_entities.first() {
        match state.target_metadata.get(first_entity) {
            Some(Resource::Success(metadata)) => &metadata.fields,
            _ => {
                sheet.write_string(row, 0, "No target metadata loaded")?;
                sheet.autofit();
                return Ok(());
            }
        }
    } else {
        sheet.write_string(row, 0, "No target entities")?;
        sheet.autofit();
        return Ok(());
    };

    // Create a lookup map for target field types
    let target_field_types: std::collections::HashMap<_, _> = target_fields
        .iter()
        .map(|f| (f.logical_name.as_str(), format!("{:?}", f.field_type)))
        .collect();

    // Partition fields into mapped, unmapped, and ignored
    let mut mapped_fields = Vec::new();
    let mut unmapped_fields = Vec::new();
    let mut ignored_fields = Vec::new();

    let source_entity_name = first_source_entity.unwrap();

    for field in source_fields {
        // Construct field key: qualified in multi-entity mode, simple otherwise
        let field_key = make_field_key(source_entity_name, &field.logical_name);

        // Check ignore status - try both qualified and unqualified ignore IDs
        let ignore_id_simple = format!("fields:source:{}", field.logical_name);
        let ignore_id_qualified = format!("fields:source:{}", field_key);

        if state.ignored_items.contains(&ignore_id_simple) || state.ignored_items.contains(&ignore_id_qualified) {
            ignored_fields.push(field);
        } else if state.field_matches.contains_key(&field_key) {
            mapped_fields.push(field);
        } else {
            unmapped_fields.push(field);
        }
    }

    // MAPPED FIELDS Section
    if !mapped_fields.is_empty() {
        sheet.write_string_with_format(row, 0, "✓ MAPPED FIELDS", &header_format)?;
        row += 1;

        // Group by match type (using primary target's match type)
        let exact_matches: Vec<_> = mapped_fields
            .iter()
            .filter(|f| {
                let field_key = make_field_key(source_entity_name, &f.logical_name);
                state.field_matches.get(&field_key)
                    .and_then(|m| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Exact)
                    .unwrap_or(false)
            })
            .collect();

        let manual_mappings: Vec<_> = mapped_fields
            .iter()
            .filter(|f| {
                let field_key = make_field_key(source_entity_name, &f.logical_name);
                state.field_matches.get(&field_key)
                    .and_then(|m| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Manual)
                    .unwrap_or(false)
            })
            .collect();

        let prefix_matches: Vec<_> = mapped_fields
            .iter()
            .filter(|f| {
                let field_key = make_field_key(source_entity_name, &f.logical_name);
                state.field_matches.get(&field_key)
                    .and_then(|m| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Prefix)
                    .unwrap_or(false)
            })
            .collect();

        let type_mismatches: Vec<_> = mapped_fields
            .iter()
            .filter(|f| {
                let field_key = make_field_key(source_entity_name, &f.logical_name);
                state.field_matches.get(&field_key)
                    .and_then(|m| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| matches!(mt, MatchType::TypeMismatch(_)))
                    .unwrap_or(false)
            })
            .collect();

        let example_matches: Vec<_> = mapped_fields
            .iter()
            .filter(|f| {
                let field_key = make_field_key(source_entity_name, &f.logical_name);
                state.field_matches.get(&field_key)
                    .and_then(|m| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::ExampleValue)
                    .unwrap_or(false)
            })
            .collect();

        let import_matches: Vec<_> = mapped_fields
            .iter()
            .filter(|f| {
                let field_key = make_field_key(source_entity_name, &f.logical_name);
                state.field_matches.get(&field_key)
                    .and_then(|m| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Import)
                    .unwrap_or(false)
            })
            .collect();

        // Exact Matches
        if !exact_matches.is_empty() {
            sheet.write_string_with_format(row, 0, "  Exact Name + Type Matches", &Format::new().set_bold())?;
            row += 1;

            for field in exact_matches {
                let field_key = make_field_key(source_entity_name, &field.logical_name);
                if let Some(match_info) = state.field_matches.get(&field_key) {
                    let target_fields_str = match_info.target_fields.join(", ");
                    let target_types_str = match_info.target_fields
                        .iter()
                        .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &target_fields_str, &target_types_str, "Exact", &exact_match_format, &indent_format)?;
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
                let field_key = make_field_key(source_entity_name, &field.logical_name);
                if let Some(match_info) = state.field_matches.get(&field_key) {
                    let target_fields_str = match_info.target_fields.join(", ");
                    let target_types_str = match_info.target_fields
                        .iter()
                        .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &target_fields_str, &target_types_str, "Manual", &manual_mapping_format, &indent_format)?;
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
                let field_key = make_field_key(source_entity_name, &field.logical_name);
                if let Some(match_info) = state.field_matches.get(&field_key) {
                    let target_fields_str = match_info.target_fields.join(", ");
                    let target_types_str = match_info.target_fields
                        .iter()
                        .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &target_fields_str, &target_types_str, "Prefix", &prefix_match_format, &indent_format)?;
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
                let field_key = make_field_key(source_entity_name, &field.logical_name);
                if let Some(match_info) = state.field_matches.get(&field_key) {
                    let target_fields_str = match_info.target_fields.join(", ");
                    let target_types_str = match_info.target_fields
                        .iter()
                        .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &target_fields_str, &target_types_str, "Type Mismatch", &type_mismatch_format, &indent_format)?;
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
                let field_key = make_field_key(source_entity_name, &field.logical_name);
                if let Some(match_info) = state.field_matches.get(&field_key) {
                    let target_fields_str = match_info.target_fields.join(", ");
                    let target_types_str = match_info.target_fields
                        .iter()
                        .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &target_fields_str, &target_types_str, "Example", &example_value_format, &indent_format)?;
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
                let field_key = make_field_key(source_entity_name, &field.logical_name);
                if let Some(match_info) = state.field_matches.get(&field_key) {
                    let target_fields_str = match_info.target_fields.join(", ");
                    let target_types_str = match_info.target_fields
                        .iter()
                        .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write_field_row(sheet, row, field, &target_fields_str, &target_types_str, "Import", &manual_mapping_format, &indent_format)?;
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
            // Check if it has a mapping (ignored but mapped)
            let field_key = make_field_key(source_entity_name, &field.logical_name);
            let (mapped_to, mapped_type, match_type) = if let Some(match_info) = state.field_matches.get(&field_key) {
                let target_fields_str = match_info.target_fields.join(", ");
                let target_types_str = match_info.target_fields
                    .iter()
                    .map(|tf| target_field_types.get(tf.as_str()).map(|s| s.as_str()).unwrap_or("Unknown"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let match_type_str = match_info
                    .primary_target()
                    .and_then(|primary| match_info.match_types.get(primary))
                    .map(|mt| format!("{:?}", mt))
                    .unwrap_or_else(|| "Unknown".to_string());
                (target_fields_str, target_types_str, match_type_str)
            } else {
                (String::new(), String::new(), "Ignored".to_string())
            };

            write_field_row(sheet, row, field, &mapped_to, &mapped_type, &match_type, &unmapped_format, &indent_format)?;
            row += 1;
        }
    }

    sheet.autofit();
    Ok(())
}
