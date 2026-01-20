//! Excel export functionality for migration analysis

pub mod csv_exporter;
mod formatting;
mod helpers;
pub mod sheets;

use anyhow::{Context, Result};
use rust_xlsxwriter::*;

use super::app::State;
use helpers::try_open_file;
use sheets::*;

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

        // Create field mapping worksheets
        create_source_fields_sheet(&mut workbook, state)?;
        create_target_fields_sheet(&mut workbook, state)?;

        // Create statistics overview last
        create_stats_sheet(&mut workbook, state)?;

        workbook
            .save(file_path)
            .with_context(|| format!("Failed to save Excel file: {}", file_path))?;

        log::info!("Excel file exported to: {}", file_path);
        Ok(())
    }
}
