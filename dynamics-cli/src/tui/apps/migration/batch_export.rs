//! Batch Excel export for all entity comparisons
//!
//! Exports all entity comparisons with stored mappings into a single Excel workbook,
//! with one sheet per entity comparison. Shows both source→target and target→source
//! perspectives to properly display N-to-1 and 1-to-N relationships.

use anyhow::{Context, Result};
use rust_xlsxwriter::*;
use std::collections::HashMap;
use std::path::PathBuf;
use sqlx::SqlitePool;
use crate::config::repository::migrations::SavedComparison;

/// Export all comparisons to a single Excel workbook
pub async fn export_all_comparisons_to_excel(
    pool: &SqlitePool,
    comparisons: &[SavedComparison],
    output_path: PathBuf,
) -> Result<()> {
    log::info!("📊 Starting batch export of {} comparisons to {:?}", comparisons.len(), output_path);
    log::debug!("📊 Comparisons: {:?}", comparisons.iter().map(|c| &c.name).collect::<Vec<_>>());

    let mut workbook = Workbook::new();
    let mut sheets_created = 0;

    // Format definitions
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let section_format = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color(Color::RGB(0xE7E6E6));

    let manual_format = Format::new()
        .set_background_color(Color::RGB(0x87CEEB));  // Sky Blue

    let import_format = Format::new()
        .set_background_color(Color::RGB(0xAFEEEE));  // Pale Turquoise/Cyan

    for comparison in comparisons {
        log::debug!("📊 Processing comparison '{}'", comparison.name);

        // Query field_mappings from database
        let manual_mappings = fetch_field_mappings(pool, &comparison.source_entity, &comparison.target_entity).await?;

        // Query imported_mappings from database
        let imported_mappings = fetch_imported_mappings(pool, &comparison.source_entity, &comparison.target_entity).await?;

        // Merge mappings: start with manual, override with imported
        let mut combined_mappings: HashMap<String, Vec<String>> = manual_mappings.clone();
        let mut mapping_types: HashMap<String, &str> = HashMap::new();

        // Mark manual mappings
        for source in manual_mappings.keys() {
            mapping_types.insert(source.clone(), "Manual");
        }

        // Override with imported (imported wins)
        for (source, targets) in &imported_mappings {
            combined_mappings.insert(source.clone(), targets.clone());
            mapping_types.insert(source.clone(), "Import");
        }

        // Skip if no mappings
        if combined_mappings.is_empty() {
            log::debug!("📊   ⏭️  Skipping comparison '{}' (no mappings)", comparison.name);
            continue;
        }

        log::info!("📊   ✅ Creating sheet for comparison '{}' ({} total, {} manual, {} imported)",
            comparison.name, combined_mappings.len(), manual_mappings.len(), imported_mappings.len());

        // Create worksheet for this comparison
        let mut sheet = workbook.add_worksheet();
        sheet.set_name(&comparison.name)?;

        let mut row: u32 = 0;

        // Title
        sheet.write_string(row, 0, &format!(
            "{} → {}",
            comparison.source_entity,
            comparison.target_entity
        ))?;
        row += 2;

        // === SOURCE PERSPECTIVE SECTION ===
        sheet.write_string_with_format(row, 0, "Source → Target Mappings", &section_format)?;
        row += 1;

        // Header row
        sheet.write_string_with_format(row, 0, "Source Field", &header_format)?;
        sheet.write_string_with_format(row, 1, "Target Fields", &header_format)?;
        sheet.write_string_with_format(row, 2, "Type", &header_format)?;
        row += 1;

        // Write combined mappings (sorted for consistency)
        let mut sorted_sources: Vec<_> = combined_mappings.keys().collect();
        sorted_sources.sort();

        for source in sorted_sources {
            let targets = &combined_mappings[source];
            let mapping_type = mapping_types.get(source).unwrap_or(&"Manual");
            let targets_str = targets.join(", ");

            let format = if *mapping_type == "Import" {
                &import_format
            } else {
                &manual_format
            };

            sheet.write_string_with_format(row, 0, source, format)?;
            sheet.write_string_with_format(row, 1, &targets_str, format)?;
            sheet.write_string_with_format(row, 2, *mapping_type, format)?;
            row += 1;
        }

        row += 2; // Blank row

        // === TARGET PERSPECTIVE SECTION ===
        sheet.write_string_with_format(row, 0, "Target → Source Mappings", &section_format)?;
        row += 1;

        // Header row
        sheet.write_string_with_format(row, 0, "Target Field", &header_format)?;
        sheet.write_string_with_format(row, 1, "Source Fields", &header_format)?;
        sheet.write_string_with_format(row, 2, "Type", &header_format)?;
        row += 1;

        // Build reverse mapping (target -> sources) from combined mappings
        let mut target_to_sources: HashMap<String, Vec<(String, &str)>> = HashMap::new();

        for (source, targets) in &combined_mappings {
            let mapping_type = mapping_types.get(source).unwrap_or(&"Manual");
            for target in targets {
                target_to_sources
                    .entry(target.clone())
                    .or_insert_with(Vec::new)
                    .push((source.clone(), *mapping_type));
            }
        }

        // Sort target fields alphabetically for consistent output
        let mut sorted_targets: Vec<_> = target_to_sources.iter().collect();
        sorted_targets.sort_by(|a, b| a.0.cmp(b.0));

        // Write reverse mappings
        for (target, source_types) in sorted_targets {
            if source_types.is_empty() {
                continue; // Skip if no sources (shouldn't happen but be safe)
            }

            // Check if all sources have the same type
            let types: Vec<&str> = source_types.iter().map(|(_, t)| *t).collect();
            let all_same_type = types.len() == 1 || types.windows(2).all(|w| w[0] == w[1]);

            let mapping_type = if all_same_type {
                types[0]
            } else {
                "Mixed" // Different sources have different types
            };

            let sources_str = source_types.iter()
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");

            let format = if mapping_type == "Import" {
                &import_format
            } else {
                &manual_format
            };

            sheet.write_string_with_format(row, 0, target, format)?;
            sheet.write_string_with_format(row, 1, &sources_str, format)?;
            sheet.write_string_with_format(row, 2, mapping_type, format)?;
            row += 1;
        }

        // Auto-size columns
        sheet.set_column_width(0, 30)?;
        sheet.set_column_width(1, 40)?;
        sheet.set_column_width(2, 12)?;

        sheets_created += 1;
    }

    if sheets_created == 0 {
        log::warn!("📊 No comparisons with mappings to export");
        anyhow::bail!("No comparisons with mappings to export");
    }

    log::info!("📊 Saving workbook with {} sheets to {:?}", sheets_created, output_path);

    // Save workbook
    workbook.save(&output_path)
        .with_context(|| format!("Failed to save Excel file to {:?}", output_path))?;

    log::info!("📊 ✅ Successfully exported {} comparison sheets to {:?}", sheets_created, output_path);

    // Try to open the file
    try_open_file(output_path.to_str().unwrap_or(""));

    Ok(())
}

/// Fetch field mappings from database for a specific entity pair
async fn fetch_field_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<HashMap<String, Vec<String>>> {
    #[derive(sqlx::FromRow)]
    struct MappingRow {
        source_field: String,
        target_field: String,
    }

    let rows: Vec<MappingRow> = sqlx::query_as(
        "SELECT source_field, target_field FROM field_mappings
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY source_field, target_field"
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to fetch field mappings")?;

    // Group by source_field to support N-to-1 mappings
    let mut mappings: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        mappings.entry(row.source_field)
            .or_insert_with(Vec::new)
            .push(row.target_field);
    }

    Ok(mappings)
}

/// Fetch imported mappings from database for a specific entity pair
async fn fetch_imported_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<HashMap<String, Vec<String>>> {
    #[derive(sqlx::FromRow)]
    struct MappingRow {
        source_field: String,
        target_field: String,
    }

    let rows: Vec<MappingRow> = sqlx::query_as(
        "SELECT source_field, target_field FROM imported_mappings
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY source_field, target_field"
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to fetch imported mappings")?;

    // Group by source_field
    let mut mappings: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        mappings.entry(row.source_field)
            .or_insert_with(Vec::new)
            .push(row.target_field);
    }

    Ok(mappings)
}

/// Try to open the Excel file with appropriate application
fn try_open_file(file_path: &str) {
    use std::process::Command;

    let result = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "start", "", file_path])
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg(file_path)
            .spawn()
    } else if cfg!(target_os = "linux") {
        Command::new("libreoffice")
            .args(["--calc", file_path])
            .spawn()
            .or_else(|_| {
                Command::new("onlyoffice-desktopeditors")
                    .arg(file_path)
                    .spawn()
            })
    } else {
        log::info!("File saved to: {}", file_path);
        return;
    };

    match result {
        Ok(_) => log::info!("Opened Excel file: {}", file_path),
        Err(e) => log::warn!("Could not auto-open file: {}. Please open manually: {}", e, file_path),
    }
}
