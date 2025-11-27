//! Excel report builder for the Entity Sync App
//!
//! Generates a comprehensive Excel report containing:
//! - Summary sheet with sync overview
//! - Manual review items (type mismatches, target-only fields)
//! - Nulled lookups details
//! - Entity operation details

use anyhow::{Context, Result};
use rust_xlsxwriter::*;

use super::super::types::{
    FieldSyncStatus, ManualReviewField, NulledLookupInfo, SyncError, SyncPlan, SyncReport,
    SyncSummary,
};

/// Build a sync report from a sync plan (before execution)
pub fn build_pre_execution_report(plan: &SyncPlan) -> SyncReport {
    let mut report = SyncReport {
        sync_date: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        origin_env: plan.origin_env.clone(),
        target_env: plan.target_env.clone(),
        synced_entities: plan
            .entity_plans
            .iter()
            .map(|p| p.entity_info.logical_name.clone())
            .collect(),
        summary: SyncSummary {
            entities_synced: plan.entity_plans.len(),
            records_deleted: plan.total_delete_count,
            records_inserted: plan.total_insert_count,
            fields_added: 0,
            fields_needing_review: 0,
            lookups_nulled: 0,
        },
        manual_review_fields: vec![],
        nulled_lookups: vec![],
        errors: vec![],
    };

    // Collect manual review items and counts
    for entity_plan in &plan.entity_plans {
        let entity_name = &entity_plan.entity_info.logical_name;

        // Count fields to add (non-system)
        let fields_to_add = entity_plan
            .schema_diff
            .fields_to_add
            .iter()
            .filter(|f| !f.is_system_field)
            .count();
        report.summary.fields_added += fields_to_add;

        // Type mismatches
        for field in &entity_plan.schema_diff.fields_type_mismatch {
            let reason = if let FieldSyncStatus::TypeMismatch {
                origin_type,
                target_type,
            } = &field.status
            {
                format!("Type mismatch: {} (origin) vs {} (target)", origin_type, target_type)
            } else {
                "Type mismatch".to_string()
            };

            report.manual_review_fields.push(ManualReviewField {
                entity_name: entity_name.clone(),
                field_name: field.logical_name.clone(),
                field_type: field.field_type.clone(),
                reason,
            });
        }

        // Target-only fields (non-system)
        for field in &entity_plan.schema_diff.fields_target_only {
            if !field.is_system_field {
                report.manual_review_fields.push(ManualReviewField {
                    entity_name: entity_name.clone(),
                    field_name: field.logical_name.clone(),
                    field_type: field.field_type.clone(),
                    reason: "Field only exists in target - consider deletion".to_string(),
                });
            }
        }

        // Nulled lookups
        for nulled in &entity_plan.nulled_lookups {
            report.nulled_lookups.push(nulled.clone());
            report.summary.lookups_nulled += 1;
        }
    }

    report.summary.fields_needing_review = report.manual_review_fields.len();

    report
}

/// Export sync report to Excel file
pub fn export_report_to_excel(report: &SyncReport, file_path: &str) -> Result<()> {
    let mut workbook = Workbook::new();

    // Create sheets
    create_summary_sheet(&mut workbook, report)?;
    create_manual_review_sheet(&mut workbook, report)?;
    create_nulled_lookups_sheet(&mut workbook, report)?;

    workbook
        .save(file_path)
        .with_context(|| format!("Failed to save Excel file: {}", file_path))?;

    log::info!("Sync report exported to: {}", file_path);
    Ok(())
}

/// Create summary sheet
fn create_summary_sheet(workbook: &mut Workbook, report: &SyncReport) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Summary")?;

    let header_format = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let title_format = Format::new().set_bold().set_font_size(16);

    let bold_format = Format::new().set_bold();

    // Title
    sheet.write_string_with_format(
        0,
        0,
        &format!(
            "Entity Sync Report: {} -> {}",
            report.origin_env, report.target_env
        ),
        &title_format,
    )?;

    sheet.write_string(1, 0, &format!("Generated: {}", report.sync_date))?;

    let mut row = 3u32;

    // Summary section
    sheet.write_string_with_format(row, 0, "SYNC SUMMARY", &header_format)?;
    row += 1;

    sheet.write_string_with_format(row, 0, "Metric", &bold_format)?;
    sheet.write_string_with_format(row, 1, "Value", &bold_format)?;
    row += 1;

    sheet.write_string(row, 0, "Entities Synced")?;
    sheet.write_number(row, 1, report.summary.entities_synced as f64)?;
    row += 1;

    sheet.write_string(row, 0, "Records Deleted")?;
    sheet.write_number(row, 1, report.summary.records_deleted as f64)?;
    row += 1;

    sheet.write_string(row, 0, "Records Inserted")?;
    sheet.write_number(row, 1, report.summary.records_inserted as f64)?;
    row += 1;

    sheet.write_string(row, 0, "Fields Added")?;
    sheet.write_number(row, 1, report.summary.fields_added as f64)?;
    row += 1;

    sheet.write_string(row, 0, "Fields Needing Review")?;
    sheet.write_number(row, 1, report.summary.fields_needing_review as f64)?;
    row += 1;

    sheet.write_string(row, 0, "Lookups Nulled")?;
    sheet.write_number(row, 1, report.summary.lookups_nulled as f64)?;
    row += 2;

    // Entities section
    sheet.write_string_with_format(row, 0, "ENTITIES SYNCED", &header_format)?;
    row += 1;

    for entity in &report.synced_entities {
        sheet.write_string(row, 0, entity)?;
        row += 1;
    }

    row += 1;

    // Errors section (if any)
    if !report.errors.is_empty() {
        sheet.write_string_with_format(row, 0, "ERRORS", &header_format)?;
        row += 1;

        sheet.write_string_with_format(row, 0, "Entity", &bold_format)?;
        sheet.write_string_with_format(row, 1, "Operation", &bold_format)?;
        sheet.write_string_with_format(row, 2, "Record ID", &bold_format)?;
        sheet.write_string_with_format(row, 3, "Error", &bold_format)?;
        row += 1;

        for error in &report.errors {
            sheet.write_string(row, 0, &error.entity_name)?;
            sheet.write_string(row, 1, &error.operation)?;
            sheet.write_string(row, 2, error.record_id.as_deref().unwrap_or("-"))?;
            sheet.write_string(row, 3, &error.error_message)?;
            row += 1;
        }
    }

    sheet.autofit();
    Ok(())
}

/// Create manual review sheet
fn create_manual_review_sheet(workbook: &mut Workbook, report: &SyncReport) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Manual Review")?;

    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let warning_format = Format::new().set_background_color(Color::RGB(0xFFC000));

    let mismatch_format = Format::new().set_background_color(Color::RGB(0xFF6B6B));

    // Headers
    sheet.write_string_with_format(0, 0, "Entity", &header_format)?;
    sheet.write_string_with_format(0, 1, "Field", &header_format)?;
    sheet.write_string_with_format(0, 2, "Type", &header_format)?;
    sheet.write_string_with_format(0, 3, "Reason", &header_format)?;
    sheet.write_string_with_format(0, 4, "Action Required", &header_format)?;

    let mut row = 1u32;

    if report.manual_review_fields.is_empty() {
        sheet.write_string(row, 0, "No fields require manual review")?;
    } else {
        for field in &report.manual_review_fields {
            let is_mismatch = field.reason.contains("mismatch");
            let format = if is_mismatch {
                &mismatch_format
            } else {
                &warning_format
            };

            sheet.write_string_with_format(row, 0, &field.entity_name, format)?;
            sheet.write_string_with_format(row, 1, &field.field_name, format)?;
            sheet.write_string_with_format(row, 2, &field.field_type, format)?;
            sheet.write_string_with_format(row, 3, &field.reason, format)?;

            let action = if is_mismatch {
                "Review type compatibility - may need manual data migration"
            } else {
                "Consider deleting from target if not needed"
            };
            sheet.write_string_with_format(row, 4, action, format)?;

            row += 1;
        }
    }

    sheet.autofit();
    Ok(())
}

/// Create nulled lookups sheet
fn create_nulled_lookups_sheet(workbook: &mut Workbook, report: &SyncReport) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Nulled Lookups")?;

    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    // Headers
    sheet.write_string_with_format(0, 0, "Entity", &header_format)?;
    sheet.write_string_with_format(0, 1, "Lookup Field", &header_format)?;
    sheet.write_string_with_format(0, 2, "Target Entity", &header_format)?;
    sheet.write_string_with_format(0, 3, "Affected Records", &header_format)?;
    sheet.write_string_with_format(0, 4, "Reason", &header_format)?;

    let mut row = 1u32;

    if report.nulled_lookups.is_empty() {
        sheet.write_string(row, 0, "No lookups were nulled")?;
    } else {
        for nulled in &report.nulled_lookups {
            sheet.write_string(row, 0, &nulled.entity_name)?;
            sheet.write_string(row, 1, &nulled.field_name)?;
            sheet.write_string(row, 2, &nulled.target_entity)?;
            sheet.write_number(row, 3, nulled.affected_count as f64)?;
            sheet.write_string(row, 4, "Target entity not in sync set")?;
            row += 1;
        }
    }

    sheet.autofit();
    Ok(())
}

/// Try to open the file with the system default application
pub fn try_open_file(file_path: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", file_path])
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(file_path).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(file_path)
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::apps::sync::types::*;

    fn make_test_plan() -> SyncPlan {
        SyncPlan {
            origin_env: "dev".to_string(),
            target_env: "test".to_string(),
            entity_plans: vec![EntitySyncPlan {
                entity_info: SyncEntityInfo {
                    logical_name: "account".to_string(),
                    display_name: Some("Account".to_string()),
                    primary_name_attribute: Some("name".to_string()),
                    category: DependencyCategory::Standalone,
                    lookups: vec![],
                    dependents: vec![],
                    insert_priority: 0,
                    delete_priority: 0,
                },
                schema_diff: EntitySchemaDiff {
                    entity_name: "account".to_string(),
                    fields_in_both: vec![],
                    fields_to_add: vec![FieldDiffEntry {
                        logical_name: "new_field".to_string(),
                        display_name: Some("New Field".to_string()),
                        field_type: "String".to_string(),
                        status: FieldSyncStatus::OriginOnly,
                        is_system_field: false,
                        origin_metadata: None,
                    }],
                    fields_target_only: vec![FieldDiffEntry {
                        logical_name: "old_field".to_string(),
                        display_name: Some("Old Field".to_string()),
                        field_type: "String".to_string(),
                        status: FieldSyncStatus::TargetOnly,
                        is_system_field: false,
                        origin_metadata: None,
                    }],
                    fields_type_mismatch: vec![FieldDiffEntry {
                        logical_name: "mismatched".to_string(),
                        display_name: Some("Mismatched".to_string()),
                        field_type: "String".to_string(),
                        status: FieldSyncStatus::TypeMismatch {
                            origin_type: "String".to_string(),
                            target_type: "Integer".to_string(),
                        },
                        is_system_field: false,
                        origin_metadata: None,
                    }],
                },
                data_preview: EntityDataPreview {
                    entity_name: "account".to_string(),
                    origin_count: 100,
                    target_count: 50,
                    origin_records: vec![],
                    target_record_ids: vec![],
                },
                nulled_lookups: vec![NulledLookupInfo {
                    entity_name: "account".to_string(),
                    field_name: "ownerid".to_string(),
                    target_entity: "systemuser".to_string(),
                    affected_count: 100,
                }],
            }],
            detected_junctions: vec![],
            has_schema_changes: true,
            total_delete_count: 50,
            total_insert_count: 100,
        }
    }

    #[test]
    fn test_build_pre_execution_report() {
        let plan = make_test_plan();
        let report = build_pre_execution_report(&plan);

        assert_eq!(report.origin_env, "dev");
        assert_eq!(report.target_env, "test");
        assert_eq!(report.synced_entities.len(), 1);
        assert_eq!(report.summary.entities_synced, 1);
        assert_eq!(report.summary.records_deleted, 50);
        assert_eq!(report.summary.records_inserted, 100);
        assert_eq!(report.summary.fields_added, 1);
        assert_eq!(report.summary.fields_needing_review, 2); // mismatch + target-only
        assert_eq!(report.summary.lookups_nulled, 1);
    }

    #[test]
    fn test_manual_review_fields() {
        let plan = make_test_plan();
        let report = build_pre_execution_report(&plan);

        assert_eq!(report.manual_review_fields.len(), 2);

        // Check type mismatch
        let mismatch = report
            .manual_review_fields
            .iter()
            .find(|f| f.field_name == "mismatched")
            .unwrap();
        assert!(mismatch.reason.contains("Type mismatch"));

        // Check target-only
        let target_only = report
            .manual_review_fields
            .iter()
            .find(|f| f.field_name == "old_field")
            .unwrap();
        assert!(target_only.reason.contains("only exists in target"));
    }

    #[test]
    fn test_nulled_lookups() {
        let plan = make_test_plan();
        let report = build_pre_execution_report(&plan);

        assert_eq!(report.nulled_lookups.len(), 1);
        assert_eq!(report.nulled_lookups[0].field_name, "ownerid");
        assert_eq!(report.nulled_lookups[0].affected_count, 100);
    }
}
