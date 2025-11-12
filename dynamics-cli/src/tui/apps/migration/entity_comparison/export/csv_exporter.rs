//! CSV export functionality for unmapped source fields

use anyhow::{Context, Result};
use csv::Writer;

use super::super::app::State;
use crate::tui::resource::Resource;

/// Export unmapped source field names to a single-column CSV file
/// In multi-entity mode, exports qualified field names (entity.field)
pub fn export_unmapped_fields_to_csv(state: &State, file_path: &str) -> Result<()> {
    let is_multi_entity = state.source_entities.len() > 1;

    // Collect unmapped fields from all source entities
    let mut unmapped_fields = Vec::new();

    for source_entity in &state.source_entities {
        // Get source fields from metadata for this entity
        let source_fields = match state.source_metadata.get(source_entity) {
            Some(Resource::Success(metadata)) => &metadata.fields,
            _ => {
                log::warn!("Source metadata for {} not loaded, skipping", source_entity);
                continue;
            }
        };

        // Filter to get unmapped fields (excluding ignored items)
        for field in source_fields {
            let field_key = if is_multi_entity {
                format!("{}.{}", source_entity, field.logical_name)
            } else {
                field.logical_name.clone()
            };

            // Field is unmapped if it's not in field_matches
            if state.field_matches.contains_key(&field_key) {
                continue;
            }

            // Check if field is ignored
            // In multi-entity mode, ignored items might be qualified
            let ignore_id_unqualified = format!("fields:source:{}", field.logical_name);
            let ignore_id_qualified = format!("fields:source:{}", field_key);
            if state.ignored_items.contains(&ignore_id_unqualified) || state.ignored_items.contains(&ignore_id_qualified) {
                continue;
            }

            unmapped_fields.push(field_key);
        }
    }

    // Sort for consistent output
    unmapped_fields.sort();

    // Create CSV writer
    let mut wtr = Writer::from_path(file_path)
        .with_context(|| format!("Failed to create CSV file: {}", file_path))?;

    // Write header
    if is_multi_entity {
        wtr.write_record(&["Field Name (Entity.Field)"])
            .context("Failed to write CSV header")?;
    } else {
        wtr.write_record(&["Field Name"])
            .context("Failed to write CSV header")?;
    }

    // Write unmapped field names
    for field_name in unmapped_fields {
        wtr.write_record(&[&field_name])
            .with_context(|| format!("Failed to write field: {}", field_name))?;
    }

    // Flush to ensure all data is written
    wtr.flush()
        .context("Failed to flush CSV writer")?;

    log::info!("CSV file exported to: {}", file_path);
    Ok(())
}
