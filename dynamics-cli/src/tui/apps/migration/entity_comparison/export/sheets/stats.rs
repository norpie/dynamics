//! Statistics sheet - mapping coverage and breakdown

use anyhow::Result;
use rust_xlsxwriter::*;

use crate::tui::Resource;
use super::super::super::app::State;
use super::super::super::MatchType;
use super::super::formatting::*;

/// Create statistics overview sheet
pub fn create_stats_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Statistics")?;

    let header_format = create_header_format();
    let title_format = create_title_format();
    let bold_format = Format::new().set_bold();
    let percent_format = Format::new().set_num_format("0.0%");

    // Title
    // TODO: Support multi-entity mode - for now use first entity
    let first_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
    let first_target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");
    sheet.write_string_with_format(
        0,
        0,
        &format!("Mapping Statistics - {} → {}", first_source_entity, first_target_entity),
        &title_format,
    )?;

    let mut row = 2u32;

    // Get field counts
    let source_fields = if let Some(first_entity) = state.source_entities.first() {
        match state.source_metadata.get(first_entity) {
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

    // ===== SOURCE STATISTICS =====
    sheet.write_string_with_format(row, 0, "SOURCE FIELDS", &header_format)?;
    row += 1;

    let source_total = source_fields.len();
    let mut source_mapped = 0;
    let mut source_unmapped = 0;
    let mut source_ignored = 0;

    // Count by match type
    let mut exact_count = 0;
    let mut manual_count = 0;
    let mut prefix_count = 0;
    let mut type_mismatch_count = 0;
    let mut example_count = 0;
    let mut import_count = 0;

    for field in source_fields {
        let ignore_id = format!("fields:source:{}", field.logical_name);

        if state.ignored_items.contains(&ignore_id) {
            source_ignored += 1;
        } else if let Some(match_info) = state.field_matches.get(&field.logical_name) {
            source_mapped += 1;

            // Count by match type (using primary target)
            if let Some(primary) = match_info.primary_target() {
                if let Some(match_type) = match_info.match_types.get(primary) {
                    match match_type {
                        MatchType::Exact => exact_count += 1,
                        MatchType::Manual => manual_count += 1,
                        MatchType::Prefix => prefix_count += 1,
                        MatchType::TypeMismatch(_) => type_mismatch_count += 1,
                        MatchType::ExampleValue => example_count += 1,
                        MatchType::Import => import_count += 1,
                    }
                }
            }
        } else {
            source_unmapped += 1;
        }
    }

    // Write source overview
    sheet.write_string_with_format(row, 0, "Category", &bold_format)?;
    sheet.write_string_with_format(row, 1, "Count", &bold_format)?;
    sheet.write_string_with_format(row, 2, "Percentage", &bold_format)?;
    row += 1;

    sheet.write_string(row, 0, "Total Fields")?;
    sheet.write_number(row, 1, source_total as f64)?;
    sheet.write_number_with_format(row, 2, 1.0, &percent_format)?;
    row += 1;

    sheet.write_string(row, 0, "Mapped")?;
    sheet.write_number(row, 1, source_mapped as f64)?;
    sheet.write_number_with_format(row, 2, source_mapped as f64 / source_total as f64, &percent_format)?;
    row += 1;

    sheet.write_string(row, 0, "Unmapped")?;
    sheet.write_number(row, 1, source_unmapped as f64)?;
    sheet.write_number_with_format(row, 2, source_unmapped as f64 / source_total as f64, &percent_format)?;
    row += 1;

    sheet.write_string(row, 0, "Ignored")?;
    sheet.write_number(row, 1, source_ignored as f64)?;
    sheet.write_number_with_format(row, 2, source_ignored as f64 / source_total as f64, &percent_format)?;
    row += 2;

    // Write match type breakdown
    sheet.write_string_with_format(row, 0, "Match Type Breakdown", &bold_format)?;
    row += 1;

    if exact_count > 0 {
        sheet.write_string(row, 0, "  Exact Matches")?;
        sheet.write_number(row, 1, exact_count as f64)?;
        sheet.write_number_with_format(row, 2, exact_count as f64 / source_total as f64, &percent_format)?;
        row += 1;
    }

    if manual_count > 0 {
        sheet.write_string(row, 0, "  Manual Mappings")?;
        sheet.write_number(row, 1, manual_count as f64)?;
        sheet.write_number_with_format(row, 2, manual_count as f64 / source_total as f64, &percent_format)?;
        row += 1;
    }

    if prefix_count > 0 {
        sheet.write_string(row, 0, "  Prefix Matches")?;
        sheet.write_number(row, 1, prefix_count as f64)?;
        sheet.write_number_with_format(row, 2, prefix_count as f64 / source_total as f64, &percent_format)?;
        row += 1;
    }

    if type_mismatch_count > 0 {
        sheet.write_string(row, 0, "  Type Mismatches")?;
        sheet.write_number(row, 1, type_mismatch_count as f64)?;
        sheet.write_number_with_format(row, 2, type_mismatch_count as f64 / source_total as f64, &percent_format)?;
        row += 1;
    }

    if example_count > 0 {
        sheet.write_string(row, 0, "  Example Matches")?;
        sheet.write_number(row, 1, example_count as f64)?;
        sheet.write_number_with_format(row, 2, example_count as f64 / source_total as f64, &percent_format)?;
        row += 1;
    }

    if import_count > 0 {
        sheet.write_string(row, 0, "  Imported Mappings")?;
        sheet.write_number(row, 1, import_count as f64)?;
        sheet.write_number_with_format(row, 2, import_count as f64 / source_total as f64, &percent_format)?;
        row += 1;
    }

    row += 2;

    // ===== TARGET STATISTICS =====
    sheet.write_string_with_format(row, 0, "TARGET FIELDS", &header_format)?;
    row += 1;

    let target_total = target_fields.len();
    let mut target_mapped = 0;
    let mut target_unmapped = 0;
    let mut target_ignored = 0;

    // Build reverse matches for target
    let mut reverse_matches = std::collections::HashSet::new();
    for (source_field, match_info) in &state.field_matches {
        for target_field in &match_info.target_fields {
            reverse_matches.insert(target_field.clone());
        }
    }

    for field in target_fields {
        let ignore_id = format!("fields:target:{}", field.logical_name);

        if state.ignored_items.contains(&ignore_id) {
            target_ignored += 1;
        } else if reverse_matches.contains(&field.logical_name) {
            target_mapped += 1;
        } else {
            target_unmapped += 1;
        }
    }

    // Write target overview
    sheet.write_string_with_format(row, 0, "Category", &bold_format)?;
    sheet.write_string_with_format(row, 1, "Count", &bold_format)?;
    sheet.write_string_with_format(row, 2, "Percentage", &bold_format)?;
    row += 1;

    sheet.write_string(row, 0, "Total Fields")?;
    sheet.write_number(row, 1, target_total as f64)?;
    sheet.write_number_with_format(row, 2, 1.0, &percent_format)?;
    row += 1;

    sheet.write_string(row, 0, "Mapped")?;
    sheet.write_number(row, 1, target_mapped as f64)?;
    sheet.write_number_with_format(row, 2, target_mapped as f64 / target_total as f64, &percent_format)?;
    row += 1;

    sheet.write_string(row, 0, "Unmapped")?;
    sheet.write_number(row, 1, target_unmapped as f64)?;
    sheet.write_number_with_format(row, 2, target_unmapped as f64 / target_total as f64, &percent_format)?;
    row += 1;

    sheet.write_string(row, 0, "Ignored")?;
    sheet.write_number(row, 1, target_ignored as f64)?;
    sheet.write_number_with_format(row, 2, target_ignored as f64 / target_total as f64, &percent_format)?;
    row += 2;

    // ===== MAPPING QUALITY INDICATORS =====
    sheet.write_string_with_format(row, 0, "MAPPING QUALITY", &header_format)?;
    row += 1;

    sheet.write_string_with_format(row, 0, "Metric", &bold_format)?;
    sheet.write_string_with_format(row, 1, "Value", &bold_format)?;
    row += 1;

    // Source coverage (mapped / total)
    let source_coverage = source_mapped as f64 / source_total as f64;
    sheet.write_string(row, 0, "Source Coverage")?;
    sheet.write_number_with_format(row, 1, source_coverage, &percent_format)?;
    row += 1;

    // Target coverage (mapped / total)
    let target_coverage = target_mapped as f64 / target_total as f64;
    sheet.write_string(row, 0, "Target Coverage")?;
    sheet.write_number_with_format(row, 1, target_coverage, &percent_format)?;
    row += 1;

    // Type mismatch ratio (type mismatches / total mapped)
    if source_mapped > 0 {
        let mismatch_ratio = type_mismatch_count as f64 / source_mapped as f64;
        sheet.write_string(row, 0, "Type Mismatch Ratio")?;
        sheet.write_number_with_format(row, 1, mismatch_ratio, &percent_format)?;
        row += 1;
    }

    // Manual mapping ratio (manual / total mapped)
    if source_mapped > 0 {
        let manual_ratio = (manual_count + import_count) as f64 / source_mapped as f64;
        sheet.write_string(row, 0, "Manual Mapping Ratio")?;
        sheet.write_number_with_format(row, 1, manual_ratio, &percent_format)?;
        row += 1;
    }

    // Automatic mapping ratio (exact + prefix / total mapped)
    if source_mapped > 0 {
        let auto_ratio = (exact_count + prefix_count) as f64 / source_mapped as f64;
        sheet.write_string(row, 0, "Automatic Mapping Ratio")?;
        sheet.write_number_with_format(row, 1, auto_ratio, &percent_format)?;
        row += 1;
    }

    sheet.autofit();
    Ok(())
}
