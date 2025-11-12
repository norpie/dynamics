//! CSV export functionality for unmapped source fields

use anyhow::{Context, Result};
use csv::Writer;

use super::super::app::State;
use crate::tui::resource::Resource;

/// Export unmapped source field names to a single-column CSV file
pub fn export_unmapped_fields_to_csv(state: &State, file_path: &str) -> Result<()> {
    // Get source fields from metadata
    // TODO: Support multi-entity mode - for now use first entity
    let source_fields = if let Some(first_entity) = state.source_entities.first() {
        match state.source_metadata.get(first_entity) {
            Some(Resource::Success(metadata)) => &metadata.fields,
            _ => {
                return Err(anyhow::anyhow!("Source metadata not loaded"));
            }
        }
    } else {
        return Err(anyhow::anyhow!("No source entities"));
    };

    // Filter to get unmapped fields (excluding ignored items)
    let unmapped_fields: Vec<_> = source_fields
        .iter()
        .filter(|field| {
            // Field is unmapped if it's not in field_matches
            if state.field_matches.contains_key(&field.logical_name) {
                return false;
            }

            // Check if field is ignored (format: "fields:source:field_name")
            let ignore_id = format!("fields:source:{}", field.logical_name);
            !state.ignored_items.contains(&ignore_id)
        })
        .collect();

    // Create CSV writer
    let mut wtr = Writer::from_path(file_path)
        .with_context(|| format!("Failed to create CSV file: {}", file_path))?;

    // Write header
    wtr.write_record(&["Field Name"])
        .context("Failed to write CSV header")?;

    // Write unmapped field names
    for field in unmapped_fields {
        wtr.write_record(&[&field.logical_name])
            .with_context(|| format!("Failed to write field: {}", field.logical_name))?;
    }

    // Flush to ensure all data is written
    wtr.flush()
        .context("Failed to flush CSV writer")?;

    log::info!("CSV file exported to: {}", file_path);
    Ok(())
}
