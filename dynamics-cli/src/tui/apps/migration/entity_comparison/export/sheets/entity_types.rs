//! Source and Target Entity Types sheets (entity type mapping from relationships)

use anyhow::Result;
use rust_xlsxwriter::*;

use super::super::super::app::State;
use super::super::super::models::{MatchInfo, MatchType};
use super::super::formatting::*;

pub fn create_source_entities_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Source Entities")?;

    let header_format = create_header_format();
    let title_format = create_title_format();

    sheet.write_string_with_format(
        0,
        0,
        &format!("{} - Related Entities", state.source_entity),
        &title_format,
    )?;

    let headers = ["Entity Name", "Usage Count", "Mapped To", "Match Type"];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_string_with_format(2, col as u16, *header, &header_format)?;
    }

    let mut row = 3u32;
    let exact_match_format = create_exact_match_format();
    let manual_mapping_format = create_manual_mapping_format();
    let prefix_match_format = create_prefix_match_format();
    let example_value_format = create_example_value_format();
    let unmapped_format = create_unmapped_format();
    let indent_format = Format::new().set_indent(1);

    if state.source_entities.is_empty() {
        sheet.write_string(row, 0, "No related entities found")?;
    } else {
        let (mapped_entities, unmapped_entities): (Vec<_>, Vec<_>) = state.source_entities
            .iter()
            .partition(|(entity_name, _)| {
                let entity_id = format!("entity_{}", entity_name);
                state.entity_matches.contains_key(&entity_id)
            });

        // Mapped Entities
        if !mapped_entities.is_empty() {
            sheet.write_string_with_format(row, 0, "✓ MAPPED ENTITIES", &header_format)?;
            row += 1;

            for (entity_name, usage_count) in mapped_entities {
                let entity_id = format!("entity_{}", entity_name);
                if let Some(match_info) = state.entity_matches.get(&entity_id) {
                    // Get primary target's match type
                    let primary_match_type = match_info.primary_target()
                        .and_then(|primary| match_info.match_types.get(primary))
                        .cloned()
                        .unwrap_or(MatchType::Manual);

                    let (mapping_type, format) = match &primary_match_type {
                        MatchType::Exact => ("Exact", &exact_match_format),
                        MatchType::Manual => ("Manual", &manual_mapping_format),
                        MatchType::Import => ("Import", &manual_mapping_format),
                        MatchType::Prefix => ("Prefix", &prefix_match_format),
                        MatchType::ExampleValue => ("Example", &example_value_format),
                        MatchType::TypeMismatch(_) => ("Type Mismatch", &unmapped_format),
                    };

                    let target_fields_str = match_info.target_fields.join(", ");

                    sheet.write_string_with_format(row, 0, &format!("    {}", entity_name), &indent_format)?;
                    sheet.write_string_with_format(row, 1, &usage_count.to_string(), format)?;
                    sheet.write_string_with_format(row, 2, &target_fields_str, format)?;
                    sheet.write_string_with_format(row, 3, mapping_type, format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Unmapped Entities
        if !unmapped_entities.is_empty() {
            sheet.write_string_with_format(row, 0, "⚠ UNMAPPED ENTITIES", &header_format)?;
            row += 1;

            for (entity_name, usage_count) in unmapped_entities {
                sheet.write_string_with_format(row, 0, &format!("    {}", entity_name), &indent_format)?;
                sheet.write_string_with_format(row, 1, &usage_count.to_string(), &unmapped_format)?;
                sheet.write_string_with_format(row, 2, "", &unmapped_format)?;
                sheet.write_string_with_format(row, 3, "Unmapped", &unmapped_format)?;
                row += 1;
            }
        }
    }

    sheet.autofit();
    Ok(())
}

/// Create target entities sheet (NEW - not in legacy version)
pub fn create_target_entities_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Target Entities")?;

    let header_format = create_header_format();
    let title_format = create_title_format();

    sheet.write_string_with_format(
        0,
        0,
        &format!("{} - Related Entities", state.target_entity),
        &title_format,
    )?;

    let headers = ["Entity Name", "Usage Count", "Mapped From", "Match Type"];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_string_with_format(2, col as u16, *header, &header_format)?;
    }

    let mut row = 3u32;
    let exact_match_format = create_exact_match_format();
    let manual_mapping_format = create_manual_mapping_format();
    let prefix_match_format = create_prefix_match_format();
    let example_value_format = create_example_value_format();
    let unmapped_format = create_unmapped_format();
    let indent_format = Format::new().set_indent(1);

    // Reverse lookup for entities
    let mut reverse_entity_matches: std::collections::HashMap<String, (String, MatchInfo)> = std::collections::HashMap::new();
    for (source_entity_id, match_info) in &state.entity_matches {
        // Use primary target for reverse lookup
        if let Some(primary_target) = match_info.primary_target() {
            let target_entity_name = primary_target.strip_prefix("entity_").unwrap_or(primary_target);
            reverse_entity_matches.insert(target_entity_name.to_string(), (source_entity_id.clone(), match_info.clone()));
        }
    }

    if state.target_entities.is_empty() {
        sheet.write_string(row, 0, "No related entities found")?;
    } else {
        let (mapped_entities, unmapped_entities): (Vec<_>, Vec<_>) = state.target_entities
            .iter()
            .partition(|(entity_name, _)| reverse_entity_matches.contains_key(entity_name));

        // Mapped Entities
        if !mapped_entities.is_empty() {
            sheet.write_string_with_format(row, 0, "✓ MAPPED ENTITIES", &header_format)?;
            row += 1;

            for (entity_name, usage_count) in mapped_entities {
                if let Some((source_id, match_info)) = reverse_entity_matches.get(entity_name) {
                    let source_name = source_id.strip_prefix("entity_").unwrap_or(source_id);

                    // Get primary target's match type
                    let primary_match_type = match_info.primary_target()
                        .and_then(|primary| match_info.match_types.get(primary))
                        .cloned()
                        .unwrap_or(MatchType::Manual);

                    let (mapping_type, format) = match &primary_match_type {
                        MatchType::Exact => ("Exact", &exact_match_format),
                        MatchType::Manual => ("Manual", &manual_mapping_format),
                        MatchType::Import => ("Import", &manual_mapping_format),
                        MatchType::Prefix => ("Prefix", &prefix_match_format),
                        MatchType::ExampleValue => ("Example", &example_value_format),
                        MatchType::TypeMismatch(_) => ("Type Mismatch", &unmapped_format),
                    };

                    sheet.write_string_with_format(row, 0, &format!("    {}", entity_name), &indent_format)?;
                    sheet.write_string_with_format(row, 1, &usage_count.to_string(), format)?;
                    sheet.write_string_with_format(row, 2, source_name, format)?;
                    sheet.write_string_with_format(row, 3, mapping_type, format)?;
                    row += 1;
                }
            }
            row += 1;
        }

        // Unmapped Entities
        if !unmapped_entities.is_empty() {
            sheet.write_string_with_format(row, 0, "⚠ UNMAPPED ENTITIES", &header_format)?;
            row += 1;

            for (entity_name, usage_count) in unmapped_entities {
                sheet.write_string_with_format(row, 0, &format!("    {}", entity_name), &indent_format)?;
                sheet.write_string_with_format(row, 1, &usage_count.to_string(), &unmapped_format)?;
                sheet.write_string_with_format(row, 2, "", &unmapped_format)?;
                sheet.write_string_with_format(row, 3, "Unmapped", &unmapped_format)?;
                row += 1;
            }
        }
    }

    sheet.autofit();
    Ok(())
}

// Placeholder for examples sheets - will implement separately due to size
