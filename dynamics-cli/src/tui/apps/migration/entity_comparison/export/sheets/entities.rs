//! Source and Target Entity sheets - field mapping details

use anyhow::Result;
use rust_xlsxwriter::*;
use std::collections::HashMap;

use crate::api::metadata::FieldMetadata;
use crate::tui::Resource;
use super::super::super::app::State;
use super::super::super::{MatchInfo, MatchType};
use super::super::formatting::*;
use super::super::helpers::write_field_row;

/// Create source entity detail sheet with mapping information
pub fn create_source_entity_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
        let sheet = workbook.add_worksheet();
        sheet.set_name("Source Entity")?;

        let header_format = create_header_format();
        let title_format = create_title_format();

        // Title
        sheet.write_string_with_format(
            0,
            0,
            &format!("{} ({})", state.source_entity, state.source_env),
            &title_format,
        )?;

        // Headers
        let headers = ["Field Name", "Type", "Required", "Primary Key", "Mapped To", "Mapping Type"];
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
        let required_format = create_required_format();
        let indent_format = Format::new().set_indent(1);

        // Get source fields
        let source_fields = match &state.source_metadata {
            Resource::Success(metadata) => &metadata.fields,
            _ => {
                sheet.write_string(row, 0, "No metadata loaded")?;
                sheet.autofit();
                return Ok(());
            }
        };

        // Group fields by mapping status
        let (mapped_fields, unmapped_fields): (Vec<_>, Vec<_>) = source_fields
            .iter()
            .partition(|field| state.field_matches.contains_key(&field.logical_name));

        // Mapped Fields Section
        if !mapped_fields.is_empty() {
            sheet.write_string_with_format(row, 0, "✓ MAPPED FIELDS", &header_format)?;
            row += 1;

            // Group by match type (using primary target's match type)
            let exact_matches: Vec<_> = mapped_fields
                .iter()
                .filter(|f| {
                    state.field_matches.get(&f.logical_name)
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
                    state.field_matches.get(&f.logical_name)
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
                    state.field_matches.get(&f.logical_name)
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
                    state.field_matches.get(&f.logical_name)
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
                    state.field_matches.get(&f.logical_name)
                        .and_then(|m| {
                            m.primary_target().and_then(|primary| m.match_types.get(primary))
                        })
                        .map(|mt| mt == &MatchType::ExampleValue)
                        .unwrap_or(false)
                })
                .collect();

            // Exact Matches
            if !exact_matches.is_empty() {
                sheet.write_string_with_format(row, 0, "  Exact Name + Type Matches", &Format::new().set_bold())?;
                row += 1;

                for field in exact_matches {
                    if let Some(match_info) = state.field_matches.get(&field.logical_name) {
                        let target_fields_str = match_info.target_fields.join(", ");
                        write_field_row(sheet, row, field, &target_fields_str, "Exact", &exact_match_format, &indent_format)?;
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
                    if let Some(match_info) = state.field_matches.get(&field.logical_name) {
                        let target_fields_str = match_info.target_fields.join(", ");
                        write_field_row(sheet, row, field, &target_fields_str, "Manual", &manual_mapping_format, &indent_format)?;
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
                    if let Some(match_info) = state.field_matches.get(&field.logical_name) {
                        let target_fields_str = match_info.target_fields.join(", ");
                        write_field_row(sheet, row, field, &target_fields_str, "Prefix", &prefix_match_format, &indent_format)?;
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
                    if let Some(match_info) = state.field_matches.get(&field.logical_name) {
                        let target_fields_str = match_info.target_fields.join(", ");
                        write_field_row(sheet, row, field, &target_fields_str, "Type Mismatch", &type_mismatch_format, &indent_format)?;
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
                    if let Some(match_info) = state.field_matches.get(&field.logical_name) {
                        let target_fields_str = match_info.target_fields.join(", ");
                        write_field_row(sheet, row, field, &target_fields_str, "Example", &example_value_format, &indent_format)?;
                        row += 1;
                    }
                }
                row += 1;
            }
        }

        // Unmapped Fields Section
        if !unmapped_fields.is_empty() {
            sheet.write_string_with_format(row, 0, "⚠ UNMAPPED FIELDS", &header_format)?;
            row += 1;

            // Group unmapped by characteristics
            let required_fields: Vec<_> = unmapped_fields.iter().filter(|f| f.is_required).collect();
            let primary_key_fields: Vec<_> = unmapped_fields.iter().filter(|f| f.is_primary_key && !f.is_required).collect();
            let other_fields: Vec<_> = unmapped_fields.iter().filter(|f| !f.is_required && !f.is_primary_key).collect();

            // Required Unmapped
            if !required_fields.is_empty() {
                sheet.write_string_with_format(row, 0, "  Required Fields (Need Attention)", &Format::new().set_bold().set_font_color(Color::Red))?;
                row += 1;
                for field in required_fields {
                    write_field_row(sheet, row, field, "", "Unmapped", &required_format, &indent_format)?;
                    row += 1;
                }
                row += 1;
            }

            // Primary Keys
            if !primary_key_fields.is_empty() {
                sheet.write_string_with_format(row, 0, "  Primary Key Fields", &Format::new().set_bold())?;
                row += 1;
                for field in primary_key_fields {
                    write_field_row(sheet, row, field, "", "Unmapped", &unmapped_format, &indent_format)?;
                    row += 1;
                }
                row += 1;
            }

            // Other Fields
            if !other_fields.is_empty() {
                sheet.write_string_with_format(row, 0, "  Other Fields", &Format::new().set_bold())?;
                row += 1;
                for field in other_fields {
                    write_field_row(sheet, row, field, "", "Unmapped", &unmapped_format, &indent_format)?;
                    row += 1;
                }
            }
        }

        sheet.autofit();
        Ok(())
    }

/// Create target entity detail sheet with mapping information
pub fn create_target_entity_sheet(workbook: &mut Workbook, state: &State) -> Result<()> {
        let sheet = workbook.add_worksheet();
        sheet.set_name("Target Entity")?;

        let header_format = create_header_format();
        let title_format = create_title_format();

        sheet.write_string_with_format(
            0,
            0,
            &format!("{} ({})", state.target_entity, state.target_env),
            &title_format,
        )?;

        let headers = ["Field Name", "Type", "Required", "Primary Key", "Mapped From", "Mapping Type"];
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
        let required_format = create_required_format();
        let indent_format = Format::new().set_indent(1);

        let target_fields = match &state.target_metadata {
            Resource::Success(metadata) => &metadata.fields,
            _ => {
                sheet.write_string(row, 0, "No metadata loaded")?;
                sheet.autofit();
                return Ok(());
            }
        };

        // Reverse lookup: find source fields that map to each target field
        // For 1-to-N mappings, a target could have multiple sources mapping to it
        let mut reverse_matches: std::collections::HashMap<String, Vec<(String, MatchInfo)>> = std::collections::HashMap::new();
        for (source_field, match_info) in &state.field_matches {
            // For each target in this match_info, add a reverse mapping
            for target_field in &match_info.target_fields {
                let match_type = match_info.match_types.get(target_field).cloned().unwrap_or(MatchType::Manual);
                let confidence = match_info.confidences.get(target_field).copied().unwrap_or(1.0);
                let single_match = MatchInfo::single(source_field.clone(), match_type, confidence);

                reverse_matches.entry(target_field.clone())
                    .or_insert_with(Vec::new)
                    .push((source_field.clone(), single_match));
            }
        }

        let (mapped_fields, unmapped_fields): (Vec<_>, Vec<_>) = target_fields
            .iter()
            .partition(|field| reverse_matches.contains_key(&field.logical_name));

        // Mapped Fields Section
        if !mapped_fields.is_empty() {
            sheet.write_string_with_format(row, 0, "✓ MAPPED FIELDS", &header_format)?;
            row += 1;

            let exact_matches: Vec<_> = mapped_fields.iter().filter(|f| {
                reverse_matches.get(&f.logical_name)
                    .and_then(|sources| sources.first())
                    .and_then(|(_, m)| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Exact)
                    .unwrap_or(false)
            }).collect();

            let manual_mappings: Vec<_> = mapped_fields.iter().filter(|f| {
                reverse_matches.get(&f.logical_name)
                    .and_then(|sources| sources.first())
                    .and_then(|(_, m)| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Manual)
                    .unwrap_or(false)
            }).collect();

            let prefix_matches: Vec<_> = mapped_fields.iter().filter(|f| {
                reverse_matches.get(&f.logical_name)
                    .and_then(|sources| sources.first())
                    .and_then(|(_, m)| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| mt == &MatchType::Prefix)
                    .unwrap_or(false)
            }).collect();

            let type_mismatches: Vec<_> = mapped_fields.iter().filter(|f| {
                reverse_matches.get(&f.logical_name)
                    .and_then(|sources| sources.first())
                    .and_then(|(_, m)| {
                        m.primary_target().and_then(|primary| m.match_types.get(primary))
                    })
                    .map(|mt| matches!(mt, MatchType::TypeMismatch(_)))
                    .unwrap_or(false)
            }).collect();

            if !exact_matches.is_empty() {
                sheet.write_string_with_format(row, 0, "  Exact Name + Type Matches", &Format::new().set_bold())?;
                row += 1;
                for field in exact_matches {
                    if let Some(sources) = reverse_matches.get(&field.logical_name) {
                        let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                        let source_names_str = source_names.join(", ");
                        write_field_row(sheet, row, field, &source_names_str, "Exact", &exact_match_format, &indent_format)?;
                        row += 1;
                    }
                }
                row += 1;
            }

            if !manual_mappings.is_empty() {
                sheet.write_string_with_format(row, 0, "  Manual Mappings", &Format::new().set_bold())?;
                row += 1;
                for field in manual_mappings {
                    if let Some(sources) = reverse_matches.get(&field.logical_name) {
                        let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                        let source_names_str = source_names.join(", ");
                        write_field_row(sheet, row, field, &source_names_str, "Manual", &manual_mapping_format, &indent_format)?;
                        row += 1;
                    }
                }
                row += 1;
            }

            if !prefix_matches.is_empty() {
                sheet.write_string_with_format(row, 0, "  Prefix Matches", &Format::new().set_bold())?;
                row += 1;
                for field in prefix_matches {
                    if let Some(sources) = reverse_matches.get(&field.logical_name) {
                        let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                        let source_names_str = source_names.join(", ");
                        write_field_row(sheet, row, field, &source_names_str, "Prefix", &prefix_match_format, &indent_format)?;
                        row += 1;
                    }
                }
                row += 1;
            }

            if !type_mismatches.is_empty() {
                sheet.write_string_with_format(row, 0, "  Type Mismatches", &Format::new().set_bold().set_font_color(Color::RGB(0xFF8C00)))?;
                row += 1;
                for field in type_mismatches {
                    if let Some(sources) = reverse_matches.get(&field.logical_name) {
                        let source_names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
                        let source_names_str = source_names.join(", ");
                        write_field_row(sheet, row, field, &source_names_str, "Type Mismatch", &type_mismatch_format, &indent_format)?;
                        row += 1;
                    }
                }
                row += 1;
            }
        }

        // Unmapped Fields Section
        if !unmapped_fields.is_empty() {
            sheet.write_string_with_format(row, 0, "⚠ UNMAPPED FIELDS", &header_format)?;
            row += 1;

            let required_fields: Vec<_> = unmapped_fields.iter().filter(|f| f.is_required).collect();
            let primary_key_fields: Vec<_> = unmapped_fields.iter().filter(|f| f.is_primary_key && !f.is_required).collect();
            let other_fields: Vec<_> = unmapped_fields.iter().filter(|f| !f.is_required && !f.is_primary_key).collect();

            if !required_fields.is_empty() {
                sheet.write_string_with_format(row, 0, "  Required Fields (Need Attention)", &Format::new().set_bold().set_font_color(Color::Red))?;
                row += 1;
                for field in required_fields {
                    write_field_row(sheet, row, field, "", "Unmapped", &required_format, &indent_format)?;
                    row += 1;
                }
                row += 1;
            }

            if !primary_key_fields.is_empty() {
                sheet.write_string_with_format(row, 0, "  Primary Key Fields", &Format::new().set_bold())?;
                row += 1;
                for field in primary_key_fields {
                    write_field_row(sheet, row, field, "", "Unmapped", &unmapped_format, &indent_format)?;
                    row += 1;
                }
                row += 1;
            }

            if !other_fields.is_empty() {
                sheet.write_string_with_format(row, 0, "  Other Fields", &Format::new().set_bold())?;
                row += 1;
                for field in other_fields {
                    write_field_row(sheet, row, field, "", "Unmapped", &unmapped_format, &indent_format)?;
                    row += 1;
                }
            }
        }

        sheet.autofit();
        Ok(())
    }

