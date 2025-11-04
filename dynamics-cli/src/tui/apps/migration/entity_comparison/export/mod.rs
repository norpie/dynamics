//! Excel export functionality for migration analysis

mod formatting;
mod helpers;
pub mod sheets;
pub mod csv_exporter;

use anyhow::{Context, Result};
use rust_xlsxwriter::*;

use super::app::State;
use sheets::*;
use helpers::try_open_file;

/// Excel export functionality for migration analysis
pub struct MigrationExporter;

impl MigrationExporter {
    /// Export migration analysis to Excel file and auto-open
    pub fn export_and_open(state: &State, file_path: &str) -> Result<()> {
        Self::export_to_excel(state, file_path)?;
        try_open_file(file_path);
        Ok(())
    }

    /// Export migration analysis to Excel file
    pub fn export_to_excel(state: &State, file_path: &str) -> Result<()> {
        let mut workbook = Workbook::new();

        // Create all worksheets
        create_source_entity_sheet(&mut workbook, state)?;
        create_target_entity_sheet(&mut workbook, state)?;
        create_source_relationships_sheet(&mut workbook, state)?;
        create_target_relationships_sheet(&mut workbook, state)?;
        create_source_views_sheet(&mut workbook, state)?;
        create_target_views_sheet(&mut workbook, state)?;
        create_source_forms_sheet(&mut workbook, state)?;
        create_target_forms_sheet(&mut workbook, state)?;
        create_source_entities_sheet(&mut workbook, state)?;
        create_target_entities_sheet(&mut workbook, state)?;
        create_examples_sheet(&mut workbook, state)?;
        create_source_examples_sheet(&mut workbook, state)?;
        create_target_examples_sheet(&mut workbook, state)?;

        workbook
            .save(file_path)
            .with_context(|| format!("Failed to save Excel file: {}", file_path))?;

        log::info!("Excel file exported to: {}", file_path);
        Ok(())
    }
}
