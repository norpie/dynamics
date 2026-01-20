//! Helper functions for Excel export

use anyhow::Result;
use rust_xlsxwriter::*;
use std::process::Command;

use super::formatting::create_required_format;
use crate::api::metadata::FieldMetadata;

/// Write a field row with consistent formatting
pub fn write_field_row(
    sheet: &mut Worksheet,
    row: u32,
    field: &FieldMetadata,
    target_field_name: &str,
    mapping_type: &str,
    row_format: &Format,
    indent_format: &Format,
) -> Result<()> {
    let required_format = create_required_format();

    let name_format = if field.is_required {
        &required_format
    } else {
        indent_format
    };
    let required_cell_format = if field.is_required {
        &required_format
    } else {
        row_format
    };

    sheet.write_string_with_format(row, 0, &format!("    {}", field.logical_name), name_format)?;
    sheet.write_string_with_format(row, 1, &format!("{:?}", field.field_type), row_format)?;
    sheet.write_string_with_format(
        row,
        2,
        if field.is_required { "Yes" } else { "No" },
        required_cell_format,
    )?;
    sheet.write_string_with_format(
        row,
        3,
        if field.is_primary_key { "Yes" } else { "No" },
        row_format,
    )?;
    sheet.write_string_with_format(row, 4, target_field_name, row_format)?;
    sheet.write_string_with_format(row, 5, mapping_type, row_format)?;

    Ok(())
}

/// Try to open the Excel file with appropriate application
pub fn try_open_file(file_path: &str) {
    let result = if cfg!(target_os = "windows") {
        // Windows: use cmd /c start to open with default Excel application
        Command::new("cmd")
            .args(["/c", "start", "", file_path]) // Empty string after start is for window title
            .spawn()
    } else if cfg!(target_os = "macos") {
        // macOS: use open command
        Command::new("open").arg(file_path).spawn()
    } else if cfg!(target_os = "linux") {
        // Linux: Try LibreOffice Calc first, then onlyoffice-desktopeditors, then fallback to xdg-open
        Command::new("libreoffice")
            .args(["--calc", file_path])
            .spawn()
            .or_else(|_| {
                Command::new("onlyoffice-desktopeditors")
                    .arg(file_path)
                    .spawn()
            })
            .or_else(|_| Command::new("xdg-open").arg(file_path).spawn())
    } else {
        // Fallback for other platforms
        Command::new("xdg-open").arg(file_path).spawn()
    };

    match result {
        Ok(_) => log::info!("Opened Excel file: {}", file_path),
        Err(e) => log::warn!(
            "Could not auto-open file: {}. Please open manually: {}",
            e,
            file_path
        ),
    }
}
