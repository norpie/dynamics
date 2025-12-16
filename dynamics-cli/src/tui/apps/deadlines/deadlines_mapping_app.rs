use crossterm::event::KeyCode;
use std::path::PathBuf;
use std::collections::HashMap;
use crate::tui::{App, AppId, Command, Element, Subscription, Theme, LayeredView, Resource, FocusId};
use crate::tui::element::LayoutConstraint::*;
use crate::tui::widgets::{SelectField, SelectEvent, ListState};
use crate::{col, spacer};
use ratatui::text::{Line, Span};
use ratatui::style::{Style, Stylize};
use calamine::{Reader, open_workbook, Xlsx};

use super::field_mappings;
use super::models::{TransformedDeadline, DeadlineLookupMap, DeadlineMode};
use super::existing_deadlines::fetch_existing_deadlines;
use super::diff::match_all_deadlines;

/// Extract the primary ID field from a record
fn extract_id_from_record(record: &serde_json::Value, entity_name: &str) -> Option<String> {
    // Try common ID field patterns
    let id_fields = vec![
        format!("{}id", entity_name),  // e.g., cgk_deadlineid
        "systemuserid".to_string(),    // Special case for systemuser
        format!("{}_id", entity_name),
    ];

    for field in id_fields {
        if let Some(id_value) = record.get(&field) {
            if let Some(id_str) = id_value.as_str() {
                return Some(id_str.to_string());
            }
        }
    }

    None
}

/// Resolve a lookup field to its GUID
fn resolve_lookup(
    cache: &HashMap<String, Vec<serde_json::Value>>,
    board_meeting_lookup: &HashMap<chrono::NaiveDate, (String, String)>,
    target_entity: &str,
    cell_value: &str,
    excel_column: &str,
) -> Option<String> {
    // Special case: Board meeting (date-based lookup)
    if excel_column.to_lowercase().contains("raad") && excel_column.to_lowercase().contains("bestuur") {
        if let Ok(date) = parse_excel_date(cell_value) {
            return board_meeting_lookup.get(&date).map(|(id, _name)| id.clone());
        }
        return None;
    }

    // Regular name-based lookup
    if let Some(records) = cache.get(target_entity) {
        for record in records {
            if let Some(name) = extract_name_from_record_with_entity(record, Some(target_entity)) {
                if name.trim().to_lowercase() == cell_value.trim().to_lowercase() {
                    // Extract the ID field from the record
                    return extract_id_from_record(record, target_entity);
                }
            }
        }
    }

    None
}

/// Sanitize field values from Excel
/// - Replaces `: ` with ` - ` (colons cause issues in SharePoint paths)
fn sanitize_field_value(value: &str) -> String {
    value.replace(": ", " - ")
}

/// Parse a time value and return NaiveTime
fn parse_time(value: &str) -> Result<chrono::NaiveTime, String> {
    // Try parsing as Excel time fraction (0.0 to 1.0)
    if let Ok(fraction) = value.parse::<f64>() {
        if (0.0..=1.0).contains(&fraction) {
            let total_seconds = (fraction * 86400.0) as u32;
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;

            return chrono::NaiveTime::from_hms_opt(hours, minutes, seconds)
                .ok_or_else(|| "Invalid time components".to_string());
        }
    }

    // Try parsing as time string (HH:MM or HH:MM:SS)
    let formats = vec![
        "%H:%M",
        "%H:%M:%S",
        "%I:%M %p",
        "%I:%M:%S %p",
    ];

    for format in formats {
        if let Ok(time) = chrono::NaiveTime::parse_from_str(value, format) {
            return Ok(time);
        }
    }

    Err("Could not parse as time".to_string())
}

/// Get checkbox relationship names for an entity type
/// NOTE: NRQ support uses a custom junction entity (nrq_deadlinesupport) and is handled separately
fn get_checkbox_relationship_names(entity_type: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    if entity_type == "cgk_deadline" {
        map.insert("cgk_support".to_string(), "cgk_deadline_cgk_support".to_string());
        map.insert("cgk_category".to_string(), "cgk_deadline_cgk_category".to_string());
        map.insert("cgk_length".to_string(), "cgk_deadline_cgk_length".to_string());
        map.insert("cgk_flemishshare".to_string(), "cgk_deadline_cgk_flemishshare".to_string());
    } else if entity_type == "nrq_deadline" {
        // NOTE: nrq_support is NOT here - it uses a custom junction entity (nrq_deadlinesupport)
        // and is handled separately via resolve_custom_junction_checkboxes()
        map.insert("nrq_category".to_string(), "nrq_deadline_nrq_category".to_string());
        map.insert("nrq_subcategory".to_string(), "nrq_deadline_nrq_subcategory".to_string());
        map.insert("nrq_flemishshare".to_string(), "nrq_deadline_nrq_flemishshare".to_string());
    }

    map
}

/// Find checkbox ID by matching header name against entity records
fn find_checkbox_id(
    cache: &HashMap<String, Vec<serde_json::Value>>,
    entity_name: &str,
    header: &str,
) -> Option<String> {
    find_checkbox_id_and_name(cache, entity_name, header).map(|(id, _name)| id)
}

/// Find checkbox ID and matched name by matching header name against entity records
/// Returns (id, matched_name) where matched_name is the canonical name from Dynamics
fn find_checkbox_id_and_name(
    cache: &HashMap<String, Vec<serde_json::Value>>,
    entity_name: &str,
    header: &str,
) -> Option<(String, String)> {
    if let Some(records) = cache.get(entity_name) {
        for record in records {
            if let Some(name) = extract_name_from_record_with_entity(record, Some(entity_name)) {
                if name.trim().to_lowercase() == header.trim().to_lowercase() {
                    if let Some(id) = extract_id_from_record(record, entity_name) {
                        return Some((id, name));
                    }
                }
            }
        }
    }
    None
}

/// Resolve checked checkbox values to their IDs and relationship names
/// NOTE: For NRQ, support checkboxes are handled separately via resolve_custom_junction_checkboxes()
fn resolve_checkboxes(
    cache: &HashMap<String, Vec<serde_json::Value>>,
    entity_type: &str,
    headers: &[String],
    row: &[calamine::Data],
    rvb_idx: usize,
) -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    let entity_prefix = if entity_type == "cgk_deadline" { "cgk_" } else { "nrq_" };

    // Get relationship name mappings
    let relationship_map = get_checkbox_relationship_names(entity_type);

    // Checkbox entity types to check
    // NOTE: For NRQ, nrq_support is NOT included here - it uses a custom junction entity
    let checkbox_entities: Vec<String> = if entity_type == "cgk_deadline" {
        vec![
            "cgk_support".to_string(),
            "cgk_category".to_string(),
            "cgk_length".to_string(),
            "cgk_flemishshare".to_string(),
        ]
    } else {
        // NRQ: nrq_support is handled separately via custom junction
        vec![
            "nrq_category".to_string(),
            "nrq_subcategory".to_string(),
            "nrq_flemishshare".to_string(),
        ]
    };

    // Process columns after RvB
    for (col_idx, header) in headers.iter().enumerate().skip(rvb_idx + 1) {
        // Skip empty headers, OPM column, and IGNORE column
        let header_upper = header.to_uppercase();
        if header.is_empty() || header_upper == "OPM" || header_upper == "IGNORE" {
            continue;
        }

        if let Some(cell) = row.get(col_idx) {
            let cell_value = cell.to_string().trim().to_lowercase();

            // Check if checkbox is checked
            if cell_value == "x" || cell_value == "1" || cell_value == "true" || cell_value == "yes" {
                // Try to find matching entity record and extract its ID
                for entity_name in &checkbox_entities {
                    if let Some(id) = find_checkbox_id(cache, entity_name, header) {
                        // Get the relationship name for this entity type
                        if let Some(relationship_name) = relationship_map.get(entity_name) {
                            result.entry(relationship_name.clone())
                                .or_insert_with(Vec::new)
                                .push(id);
                        }
                        break;
                    }
                }
            }
        }
    }

    result
}

/// Resolve custom junction checkboxes (e.g., nrq_deadlinesupport for NRQ)
/// Returns a vector of CustomJunctionRecord for entities that use custom junction tables
fn resolve_custom_junction_checkboxes(
    cache: &HashMap<String, Vec<serde_json::Value>>,
    entity_type: &str,
    headers: &[String],
    row: &[calamine::Data],
    rvb_idx: usize,
) -> Vec<super::models::CustomJunctionRecord> {
    use super::models::CustomJunctionRecord;

    let mut result = Vec::new();

    // Only NRQ deadlines use custom junction for support
    if entity_type != "nrq_deadline" {
        return result;
    }

    // Process columns after RvB looking for nrq_support matches
    for (col_idx, header) in headers.iter().enumerate().skip(rvb_idx + 1) {
        // Skip empty headers, OPM column, and IGNORE column
        let header_upper = header.to_uppercase();
        if header.is_empty() || header_upper == "OPM" || header_upper == "IGNORE" {
            continue;
        }

        if let Some(cell) = row.get(col_idx) {
            let cell_value = cell.to_string().trim().to_lowercase();

            // Check if checkbox is checked
            if cell_value == "x" || cell_value == "1" || cell_value == "true" || cell_value == "yes" {
                // Try to find matching nrq_support record
                if let Some((id, matched_name)) = find_checkbox_id_and_name(cache, "nrq_support", header) {
                    result.push(CustomJunctionRecord {
                        junction_entity: "nrq_deadlinesupport".to_string(),
                        main_entity_field: "nrq_DeadlineId".to_string(),
                        related_entity_field: "nrq_SupportId".to_string(),
                        related_entity: "nrq_support".to_string(),
                        related_id: id,
                        related_name: matched_name,
                    });
                }
            }
        }
    }

    result
}

/// Fetch entity data from cache or API
async fn fetch_entity_data(
    entity_name: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let config = crate::global_config();
    let manager = crate::client_manager();

    // Get current environment
    let environment_name = manager.get_current_environment_name().await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No environment selected".to_string())?;

    // Try cache first (24 hours) - but force refresh if cache has 0 records
    match config.get_entity_data_cache(&environment_name, entity_name, 24).await {
        Ok(Some(cached)) if !cached.is_empty() => {
            log::debug!("Using cached data for {} ({} records)", entity_name, cached.len());
            return Ok(cached);
        }
        Ok(Some(cached)) => {
            log::debug!("Cache for {} has 0 records, forcing refresh", entity_name);
        }
        Ok(None) => {
            log::debug!("No cache for {}, fetching from API", entity_name);
        }
        Err(e) => {
            log::warn!("Cache lookup failed for {}: {}", entity_name, e);
        }
    }

    // Fetch from API
    let client = manager
        .get_current_client()
        .await
        .map_err(|e| e.to_string())?;

    // Pluralize entity name for Web API (entity sets are plural)
    let plural_name = crate::api::pluralization::pluralize_entity_name(entity_name);
    log::debug!("Fetching {} (plural: {})", entity_name, plural_name);

    // Fetch records for this entity using query builder
    // Filter for active records except for systemuser (which has different state semantics)
    let mut builder = crate::api::QueryBuilder::new(&plural_name);
    if entity_name != "systemuser" {
        builder = builder.active_only();
    }
    let query = builder.build();
    log::debug!("Executing query for {}: {:?}", plural_name, query);

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to fetch {}: {}", entity_name, e))?;

    let records = result.records()
        .cloned()
        .unwrap_or_default();

    log::debug!("Fetched {} records for {}", records.len(), entity_name);

    if records.is_empty() {
        log::warn!("Entity {} returned 0 records - entity might be empty or query might need adjustment", entity_name);
    } else if let Some(first) = records.first() {
        log::debug!("First record from {}: {:?}", entity_name, first);
    }

    // Cache for future use
    let _ = config.set_entity_data_cache(&environment_name, entity_name, &records).await;

    Ok(records)
}

/// Build a lookup map of board meeting dates for fast row processing
fn build_board_meeting_lookup(state: &mut State, entity_type: &str) {
    state.board_meeting_date_lookup.clear();

    // Determine which entity contains board meetings
    let board_entity = if entity_type == "cgk_deadline" {
        "cgk_deadline" // Self-referencing for CGK
    } else {
        "nrq_boardofdirectorsmeeting" // Separate entity for NRQ
    };

    if let Some(records) = state.entity_data_cache.get(board_entity) {
        let mut name_extract_failures = 0;
        let mut names_checked = 0;
        let mut bestuur_matches = 0;
        let mut first_bestuur_sample = Vec::new();

        for record in records {
            let name = match extract_name_from_record_with_entity(record, Some(board_entity)) {
                Some(n) => n,
                None => {
                    name_extract_failures += 1;
                    continue;
                }
            };

            names_checked += 1;
            let name_lower = name.to_lowercase();

            // Normalize whitespace - replace all whitespace variants (including non-breaking spaces) with regular spaces
            let name_normalized = name_lower
                .replace('\u{00A0}', " ")  // non-breaking space
                .replace('\u{2009}', " ")  // thin space
                .replace('\u{202F}', " ")  // narrow no-break space
                .trim()
                .to_string();

            // Check if this is a board meeting record
            let is_bestuur = name_normalized.starts_with("bestuur - ")
                || name_normalized.starts_with("bestuur + algemene vergadering - ")
                || name_normalized.starts_with("raad van bestuur - ");

            if is_bestuur {
                bestuur_matches += 1;
                if first_bestuur_sample.len() < 5 {
                    first_bestuur_sample.push(name.clone());
                }
            }

            if is_bestuur {
                // Extract the date part from normalized name
                let date_start = if name_normalized.starts_with("bestuur + algemene vergadering - ") {
                    "bestuur + algemene vergadering - ".len()
                } else if name_normalized.starts_with("raad van bestuur - ") {
                    "raad van bestuur - ".len()
                } else {
                    "bestuur - ".len()
                };

                let date_part = name_normalized[date_start..].split_whitespace().next().unwrap_or("");

                // Try to parse the date
                if let Ok(date) = parse_board_meeting_date(date_part) {
                    // Get the ID field - try common field name patterns
                    let id_fields = if entity_type == "cgk_deadline" {
                        vec!["cgk_deadlineid"]
                    } else {
                        vec!["nrq_boardofdirectorsmeetingid"]
                    };

                    let mut found_id = None;
                    for id_field in id_fields {
                        if let Some(id_value) = record.get(id_field) {
                            if let Some(id_str) = id_value.as_str() {
                                found_id = Some(id_str.to_string());
                                break;
                            }
                        }
                    }

                    if let Some(id_str) = found_id {
                        state.board_meeting_date_lookup.insert(date, (id_str, name));
                    }
                }
            }
        }

        log::debug!("Board meeting lookup stats for {}:", board_entity);
        log::debug!("  Total records: {}", records.len());
        log::debug!("  Name extract failures: {}", name_extract_failures);
        log::debug!("  Names checked: {}", names_checked);
        log::debug!("  Names matching 'bestuur' pattern: {}", bestuur_matches);
        if !first_bestuur_sample.is_empty() {
            log::debug!("  Sample bestuur matches: {:?}", first_bestuur_sample);
        }
        log::debug!("  Final lookup size: {}", state.board_meeting_date_lookup.len());
    } else {
        log::warn!("Board entity '{}' not found in cache", board_entity);
    }
}

/// Parse a board meeting date from the entity name (supports various formats)
fn parse_board_meeting_date(date_str: &str) -> Result<chrono::NaiveDate, String> {
    // Try various date formats that might appear in entity names
    let formats = vec![
        "%-d/%-m/%Y",  // 3/2/2025
        "%-d/%m/%Y",   // 3/02/2025
        "%d/%-m/%Y",   // 03/2/2025
        "%d/%m/%Y",    // 03/02/2025
    ];

    for format in formats {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, format) {
            return Ok(date);
        }
    }

    Err(format!("Could not parse board meeting date: {}", date_str))
}

/// Check if a row is effectively empty (all cells are empty or whitespace)
fn is_row_empty(row: &[calamine::Data]) -> bool {
    row.iter().all(|cell| {
        let cell_str = cell.to_string();
        cell_str.trim().is_empty()
    })
}

/// Process Excel file: find header row, identify checkbox columns, transform rows
fn process_excel_file(state: &mut State) -> Command<Msg> {
    state.excel_processed = true;
    state.warnings.clear();
    state.transformed_records.clear();
    state.ignored_rows.clear();

    let file_path = state.file_path.clone();
    let sheet_name = state.sheet_name.clone();

    // Open the Excel file
    let mut workbook: Xlsx<_> = match open_workbook(&file_path) {
        Ok(wb) => wb,
        Err(e) => {
            state.warnings.push(Warning(format!("Failed to open Excel file: {}", e)));
            return Command::None;
        }
    };

    // Get the specified sheet
    let range = match workbook.worksheet_range(&sheet_name) {
        Ok(range) => range,
        Err(e) => {
            state.warnings.push(Warning(format!("Failed to read sheet '{}': {}", sheet_name, e)));
            return Command::None;
        }
    };

    // Find header row by looking for "Domein*" in first column
    let mut header_row_idx = None;
    let mut headers = Vec::new();

    for (row_idx, row) in range.rows().enumerate() {
        if let Some(first_cell) = row.first() {
            let cell_str = first_cell.to_string();
            if cell_str.contains("Domein") {
                header_row_idx = Some(row_idx);
                headers = row.iter().map(|cell| cell.to_string()).collect();
                break;
            }
        }
    }

    if header_row_idx.is_none() {
        state.warnings.push(Warning("Could not find header row (looking for 'Domein*' in first column)".to_string()));
        return Command::None;
    }

    let header_row_idx = header_row_idx.unwrap();
    log::debug!("Found header row at index {}", header_row_idx);
    log::debug!("Headers: {:?}", headers);

    // Find "Type" column - checkbox columns start after this
    let type_idx = headers.iter().position(|h| h.to_uppercase() == "TYPE");

    if type_idx.is_none() {
        state.warnings.push(Warning("Could not find 'Type' column to determine checkbox boundary".to_string()));
        return Command::None;
    }

    let type_idx = type_idx.unwrap();
    log::debug!("Found 'Type' at column index {}", type_idx);

    // All columns after "Type" are checkbox columns, except "OPM" and "IGNORE"
    let checkbox_columns: Vec<String> = headers.iter()
        .skip(type_idx + 1)
        .filter(|h| {
            let h_upper = h.to_uppercase();
            !h.is_empty() && h_upper != "OPM" && h_upper != "IGNORE"
        })
        .map(|h| h.to_string())
        .collect();

    log::debug!("Checkbox columns: {:?}", checkbox_columns);

    // Get the entity type to determine which entity prefix to use
    let entity_type = state.detected_entity.as_ref()
        .or(state.manual_override.as_ref());

    if entity_type.is_none() {
        state.warnings.push(Warning("No entity type selected".to_string()));
        return Command::None;
    }

    let entity_type = entity_type.unwrap();
    let entity_prefix = if entity_type == "cgk_deadline" { "cgk_" } else { "nrq_" };

    // Debug: log what's in the entity data cache
    log::debug!("Entity data cache keys: {:?}", state.entity_data_cache.keys().collect::<Vec<_>>());
    for (entity_name, records) in &state.entity_data_cache {
        log::debug!("Entity '{}' has {} records", entity_name, records.len());
        if let Some(first_record) = records.first() {
            log::debug!("First record keys: {:?}", first_record.as_object().map(|o| o.keys().collect::<Vec<_>>()));
        }
    }

    // Validate checkbox columns against entity data (warnings only)
    for checkbox_col in &checkbox_columns {
        // Try to find matching entity records
        let mut found = false;

        // Check all checkbox entity types for this environment
        let checkbox_entity_names = vec![
            format!("{}support", entity_prefix),
            format!("{}category", entity_prefix),
            if entity_prefix == "cgk_" { "cgk_length".to_string() } else { "nrq_subcategory".to_string() },
            format!("{}flemishshare", entity_prefix),
        ];

        log::debug!("Checking checkbox column '{}' against entities: {:?}", checkbox_col, checkbox_entity_names);

        for entity_name in checkbox_entity_names {
            if let Some(records) = state.entity_data_cache.get(&entity_name) {
                log::debug!("Checking {} records in entity '{}'", records.len(), entity_name);
                // Check if any record's name matches the checkbox column header
                for (idx, record) in records.iter().enumerate() {
                    if let Some(name_value) = extract_name_from_record_with_entity(record, Some(&entity_name)) {
                        if idx < 3 {
                            log::debug!("Record {} name: '{}'", idx, name_value);
                        }
                        if name_value.trim().to_lowercase() == checkbox_col.trim().to_lowercase() {
                            found = true;
                            log::debug!("✓ Matched checkbox column '{}' to entity '{}'", checkbox_col, entity_name);
                            break;
                        }
                    } else if idx < 3 {
                        log::debug!("Record {} has no name field, record: {:?}", idx, record);
                    }
                }
                if found {
                    break;
                }
            } else {
                log::debug!("Entity '{}' not found in cache", entity_name);
            }
        }

        if !found {
            state.warnings.push(Warning(format!("Checkbox column '{}' not found in entity data", checkbox_col)));
        }
    }

    log::debug!("Checkbox validation complete. Found {} warnings", state.warnings.len());

    // Clone entity type before mutating state
    let entity_type_owned = entity_type.to_string();

    // Build board meeting lookup for efficient row processing
    build_board_meeting_lookup(state, &entity_type_owned);

    // Find IGNORE column index if it exists
    let ignore_col_idx = headers.iter().position(|h| h.to_uppercase() == "IGNORE");

    if let Some(idx) = ignore_col_idx {
        log::debug!("Found IGNORE column at index {}", idx);
    }

    // Now process data rows and build transformed records
    let data_rows = range.rows().skip(header_row_idx + 1);

    for (row_num, row) in data_rows.enumerate() {
        let excel_row_number = header_row_idx + 2 + row_num; // +2 because: +1 for header, +1 for 1-based indexing

        // Skip rows that are effectively empty (all cells empty or whitespace)
        if is_row_empty(row) {
            log::debug!("Skipping empty row {}", excel_row_number);
            continue;
        }

        // Track rows marked to be ignored
        if let Some(ignore_idx) = ignore_col_idx {
            if let Some(cell) = row.get(ignore_idx) {
                let cell_value = cell.to_string().trim().to_string();
                if !cell_value.is_empty() {
                    log::debug!("Ignoring row {} due to IGNORE column: '{}'", excel_row_number, cell_value);
                    state.ignored_rows.push((excel_row_number, cell_value));
                    continue;
                }
            }
        }

        let transformed = process_row(state, &headers, row, excel_row_number, &entity_type_owned);

        // Add OPM notes to warnings list
        if let Some(ref notes) = transformed.notes {
            state.warnings.push(Warning(format!("Row {} [OPM]: {}", excel_row_number, notes)));
        }

        // Add row-level warnings to global warnings list
        for warning in &transformed.warnings {
            state.warnings.push(Warning(format!("Row {}: {}", excel_row_number, warning)));
        }

        state.transformed_records.push(transformed);
    }

    // Add ignored rows to warnings list
    for (row_num, reason) in &state.ignored_rows {
        state.warnings.push(Warning(format!("Row {} [IGNORED]: {}", row_num, reason)));
    }

    // Update summary stats
    state.total_rows = state.transformed_records.len();
    state.rows_with_warnings = state.transformed_records.iter()
        .filter(|r| r.has_warnings())
        .count();

    log::debug!("Transformation complete. {} rows, {} with warnings",
        state.total_rows, state.rows_with_warnings);

    Command::set_focus(FocusId::new("continue-button"))
}

/// Process a single data row - now returns a TransformedDeadline
fn process_row(
    state: &State,
    headers: &[String],
    row: &[calamine::Data],
    row_number: usize,
    entity_type: &str,
) -> TransformedDeadline {
    let mut transformed = TransformedDeadline::new(row_number);

    // Capture OPM column notes if present
    if let Some(opm_idx) = headers.iter().position(|h| h.to_uppercase() == "OPM") {
        if let Some(cell) = row.get(opm_idx) {
            let opm_value = cell.to_string().trim().to_string();
            if !opm_value.is_empty() {
                transformed.notes = Some(opm_value);
            }
        }
    }

    // Get field mappings for this entity type
    let mappings = field_mappings::get_mappings_for_entity(entity_type);

    // Determine field name prefixes
    let date_field = if entity_type == "cgk_deadline" { "cgk_date" } else { "nrq_deadlinedate" };
    let commission_date_field = if entity_type == "cgk_deadline" { "cgk_datumcommissievergadering" } else { "nrq_committeemeetingdate" };

    // Process each mapped field
    for mapping in mappings {
        // Find the column index for this field
        if let Some(col_idx) = headers.iter().position(|h| h == &mapping.excel_column) {
            if let Some(cell) = row.get(col_idx) {
                let cell_value = cell.to_string();

                // Skip empty cells
                if cell_value.trim().is_empty() {
                    // Check if this is a required field
                    if mapping.required {
                        transformed.warnings.push(format!(
                            "Required field '{}' is empty",
                            mapping.excel_column
                        ));
                    }
                    continue;
                }

                match &mapping.field_type {
                    field_mappings::FieldType::Lookup { target_entity } => {
                        // Skip if this field was already populated (for fallback mappings)
                        if transformed.lookup_fields.contains_key(&mapping.dynamics_field) {
                            continue;
                        }

                        // Resolve lookup to its GUID
                        if let Some(id) = resolve_lookup(
                            &state.entity_data_cache,
                            &state.board_meeting_date_lookup,
                            target_entity,
                            &cell_value,
                            &mapping.excel_column,
                        ) {
                            transformed.lookup_fields.insert(mapping.dynamics_field.clone(), (id, target_entity.clone()));
                        } else {
                            transformed.warnings.push(format!(
                                "Lookup '{}' not found: '{}'",
                                mapping.excel_column, cell_value
                            ));
                        }
                    }
                    field_mappings::FieldType::Date => {
                        // Parse date
                        match parse_excel_date(&cell_value) {
                            Ok(date) => {
                                // Store in appropriate field
                                if mapping.dynamics_field == date_field {
                                    transformed.deadline_date = Some(date);
                                } else if mapping.dynamics_field == commission_date_field {
                                    transformed.commission_date = Some(date);
                                }
                            }
                            Err(e) => {
                                transformed.warnings.push(format!(
                                    "Invalid date in '{}': {} (value: '{}')",
                                    mapping.excel_column, e, cell_value
                                ));
                            }
                        }
                    }
                    field_mappings::FieldType::Time => {
                        // Parse time - determine if it's deadline time or commission time based on field
                        match parse_time(&cell_value) {
                            Ok(time) => {
                                if mapping.dynamics_field == date_field {
                                    // Deadline time
                                    transformed.deadline_time = Some(time);
                                } else if mapping.dynamics_field == commission_date_field {
                                    // Commission time
                                    transformed.commission_time = Some(time);
                                }
                            }
                            Err(e) => {
                                transformed.warnings.push(format!(
                                    "Invalid time in '{}': {} (value: '{}')",
                                    mapping.excel_column, e, cell_value
                                ));
                            }
                        }
                    }
                    field_mappings::FieldType::Direct => {
                        // Direct field - sanitize and copy value
                        let sanitized_value = sanitize_field_value(&cell_value);
                        transformed.direct_fields.insert(
                            mapping.dynamics_field.clone(),
                            sanitized_value
                        );
                    }
                    field_mappings::FieldType::Picklist { options } => {
                        // Picklist field - validate and map label to integer value
                        let trimmed_value = cell_value.trim();

                        if let Some(&picklist_value) = options.get(trimmed_value) {
                            // Valid picklist value - store it
                            transformed.picklist_fields.insert(
                                mapping.dynamics_field.clone(),
                                picklist_value
                            );
                        } else {
                            // Invalid or unknown picklist value
                            // Check if it's the third invalid option (GUID label)
                            if trimmed_value.len() == 36 && trimmed_value.contains('-') {
                                transformed.warnings.push(format!(
                                    "Invalid picklist value in '{}': '{}' (appears to be a GUID - not a valid option)",
                                    mapping.excel_column, trimmed_value
                                ));
                            } else {
                                // Generate a helpful warning with expected values
                                let expected_values: Vec<String> = options.keys().cloned().collect();
                                transformed.warnings.push(format!(
                                    "Invalid value in '{}': '{}'. Expected one of: {}",
                                    mapping.excel_column,
                                    trimmed_value,
                                    expected_values.join(", ")
                                ));
                            }
                        }
                    }
                    field_mappings::FieldType::Boolean { true_value, false_value } => {
                        // Boolean field - map string to true/false
                        let trimmed_value = cell_value.trim();

                        if trimmed_value.eq_ignore_ascii_case(true_value) {
                            transformed.boolean_fields.insert(
                                mapping.dynamics_field.clone(),
                                true
                            );
                        } else if trimmed_value.eq_ignore_ascii_case(false_value) {
                            transformed.boolean_fields.insert(
                                mapping.dynamics_field.clone(),
                                false
                            );
                        } else if !trimmed_value.is_empty() {
                            transformed.warnings.push(format!(
                                "Invalid boolean value in '{}': '{}'. Expected '{}' or '{}'",
                                mapping.excel_column, trimmed_value, true_value, false_value
                            ));
                        }
                    }
                    field_mappings::FieldType::Checkbox => {
                        // Checkboxes are handled separately below
                    }
                    field_mappings::FieldType::CustomJunction { .. } => {
                        // Custom junction checkboxes are handled separately via resolve_custom_junction_checkboxes()
                    }
                }
            } else {
                // Cell not found in row
                if mapping.required {
                    transformed.warnings.push(format!(
                        "Required field '{}' is missing (column exists but cell is out of bounds)",
                        mapping.excel_column
                    ));
                }
            }
        } else {
            // Column not found in headers
            if mapping.required {
                transformed.warnings.push(format!(
                    "Required field '{}' is missing from Excel file",
                    mapping.excel_column
                ));
            }
        }
    }

    // Process checkbox columns (after "Raad van Bestuur")
    let rvb_idx = headers.iter().position(|h|
        h.to_lowercase().contains("raad") && h.to_lowercase().contains("bestuur")
    );

    if let Some(rvb_idx) = rvb_idx {
        let checkbox_relationships = resolve_checkboxes(
            &state.entity_data_cache,
            entity_type,
            headers,
            row,
            rvb_idx,
        );
        transformed.checkbox_relationships = checkbox_relationships;

        // Resolve custom junction checkboxes (e.g., nrq_deadlinesupport for NRQ)
        let custom_junction_records = resolve_custom_junction_checkboxes(
            &state.entity_data_cache,
            entity_type,
            headers,
            row,
            rvb_idx,
        );
        transformed.custom_junction_records = custom_junction_records;
    }

    transformed
}

/// Parse a date value from Excel
fn parse_date_value(value: &str) -> Result<(), String> {
    parse_excel_date(value).map(|_| ())
}

/// Parse Excel date and return NaiveDate
fn parse_excel_date(value: &str) -> Result<chrono::NaiveDate, String> {
    // Try parsing as Excel serial date number
    if let Ok(serial) = value.parse::<f64>() {
        // Excel dates start at 1900-01-01, serial 1
        if serial < 1.0 || serial > 100000.0 {
            return Err(format!("Invalid Excel date serial: {}", serial));
        }
        // Excel epoch: 1900-01-01 is serial 1
        // Base date is Dec 30, 1899, and we add the serial directly
        let base_date = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();

        if let Some(date) = base_date.checked_add_days(chrono::Days::new(serial as u64)) {
            return Ok(date);
        } else {
            return Err(format!("Date calculation overflow for serial: {}", serial));
        }
    }

    // Try parsing as date string (various formats with both / and - separators)
    let formats = vec![
        "%Y-%m-%d",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%d-%m-%Y",
        "%m-%d-%Y",
        "%Y/%m/%d",
    ];

    for format in formats {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(value, format) {
            return Ok(date);
        }
    }

    Err("Could not parse as date".to_string())
}

/// Extract name field from a record (tries common name field patterns)
fn extract_name_from_record(record: &serde_json::Value) -> Option<String> {
    extract_name_from_record_with_entity(record, None)
}

/// Extract name field from a record with entity-specific field knowledge
fn extract_name_from_record_with_entity(record: &serde_json::Value, entity_name: Option<&str>) -> Option<String> {
    // Special case: systemuser uses domainname (email) field
    if entity_name == Some("systemuser") {
        if let Some(value) = record.get("domainname") {
            if let Some(s) = value.as_str() {
                return Some(s.to_string());
            }
        }
        // Fallback to internalemailaddress
        if let Some(value) = record.get("internalemailaddress") {
            if let Some(s) = value.as_str() {
                return Some(s.to_string());
            }
        }
    }

    // Try common name field patterns
    let name_fields = vec![
        "name",
        "cgk_name",
        "nrq_name",
        "fullname",
        "cgk_fullname",
        "nrq_fullname",
        "domainname", // For systemuser fallback
    ];

    for field in name_fields {
        if let Some(value) = record.get(field) {
            if let Some(s) = value.as_str() {
                return Some(s.to_string());
            }
        }
    }

    None
}

/// Start loading entity data in parallel
fn start_entity_data_loading(state: &mut State, entity_type: &str) -> Command<Msg> {
    let cache_entities = field_mappings::get_cache_entities(entity_type);

    if cache_entities.is_empty() {
        return Command::None;
    }

    state.entity_data_loading = true;
    state.entity_data_loaded_count = 0;
    state.entity_data_total_count = cache_entities.len();

    let mut builder = Command::perform_parallel();

    for (_index, entity_name) in cache_entities.iter().enumerate() {
        let entity_name_clone = entity_name.clone();

        builder = builder.add_task(
            format!("Loading {} records", entity_name),
            async move {
                fetch_entity_data(&entity_name_clone).await
            }
        );
    }

    builder
        .with_title("Loading entity data for lookups")
        .on_complete(AppId::DeadlinesMapping)
        .cancellable(false)
        .build(move |task_index, result| {
            let typed_result = result.downcast::<Result<Vec<serde_json::Value>, String>>()
                .map(|boxed| *boxed)
                .unwrap_or_else(|_| Err("Type mismatch in task result".to_string()));
            Msg::EntityDataLoaded(task_index, typed_result)
        })
}

/// Start fetching existing deadlines for edit/update support
fn start_existing_deadlines_loading(state: &mut State, entity_type: &str) -> Command<Msg> {
    state.existing_deadlines_loading = true;
    let entity_type_owned = entity_type.to_string();

    Command::perform(
        async move {
            fetch_existing_deadlines(&entity_type_owned).await
        },
        Msg::ExistingDeadlinesLoaded
    )
}

/// Try to run matching if both Excel processing and existing deadlines loading are complete.
/// Returns true if matching was performed.
fn try_run_matching(state: &mut State) -> bool {
    // Don't re-run if already complete
    if state.matching_complete {
        return false;
    }

    // Need Excel processed
    if !state.excel_processed {
        return false;
    }

    // Wait for existing deadlines to finish loading
    if state.existing_deadlines_loading {
        return false;
    }

    // Get entity type
    let entity_type = match state.detected_entity.as_ref().or(state.manual_override.as_ref()) {
        Some(et) => et.clone(),
        None => return false,
    };

    // Run matching against existing deadlines (if available)
    if let Some(ref lookup_map) = state.existing_deadlines_lookup {
        log::info!(
            "Running matching: {} transformed records against {} existing deadlines",
            state.transformed_records.len(),
            lookup_map.len()
        );
        match_all_deadlines(&mut state.transformed_records, lookup_map, &entity_type);
    } else {
        // No existing deadlines loaded - all records are Create mode (already the default)
        log::info!(
            "No existing deadlines available - all {} records will be Create mode",
            state.transformed_records.len()
        );
    }

    // Update mode counts
    state.create_count = state.transformed_records.iter().filter(|r| r.is_create()).count();
    state.update_count = state.transformed_records.iter().filter(|r| r.is_update()).count();
    state.unchanged_count = state.transformed_records.iter().filter(|r| r.is_unchanged()).count();
    state.error_count = state.transformed_records.iter().filter(|r| r.is_error()).count();

    log::info!(
        "Matching complete: {} create, {} update, {} unchanged, {} error",
        state.create_count,
        state.update_count,
        state.unchanged_count,
        state.error_count
    );

    state.matching_complete = true;
    true
}

pub struct DeadlinesMappingApp;

// Wrapper type for warnings to implement ListItem
#[derive(Clone)]
struct Warning(String);

impl crate::tui::widgets::ListItem for Warning {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (theme.accent_primary, Some(Style::default().bg(theme.bg_surface)))
        } else {
            (theme.text_primary, None)
        };

        // Determine icon and color based on message type
        let (icon, icon_color) = if self.0.contains("[OPM]:") {
            ("ℹ ", theme.accent_info) // Info icon for OPM notes
        } else if self.0.contains("[IGNORED]:") {
            ("✖ ", theme.text_tertiary) // X icon for ignored rows
        } else {
            ("⚠ ", theme.accent_warning) // Warning icon for actual warnings
        };

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(icon, Style::default().fg(icon_color)),
            Span::styled(self.0.clone(), Style::default().fg(fg_color)),
        ]));

        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }

        builder.build()
    }
}

#[derive(Clone)]
pub struct State {
    current_environment: Option<String>,
    file_path: PathBuf,
    sheet_name: String,
    entities: Resource<Vec<String>>,
    detected_entity: Option<String>,
    manual_override: Option<String>,
    entity_selector: SelectField,
    warnings: Vec<Warning>,
    warnings_list_state: ListState,
    entity_data_loading: bool,
    entity_data_loaded_count: usize,
    entity_data_total_count: usize,
    entity_data_cache: HashMap<String, Vec<serde_json::Value>>,
    board_meeting_date_lookup: HashMap<chrono::NaiveDate, (String, String)>, // date -> (id, name)
    excel_processed: bool,
    transformed_records: Vec<TransformedDeadline>,
    ignored_rows: Vec<(usize, String)>, // (row_number, ignore_reason)
    total_rows: usize,
    rows_with_warnings: usize,
    // Existing deadlines for edit support
    existing_deadlines_loading: bool,
    existing_deadlines_lookup: Option<DeadlineLookupMap>,
    // Matching status
    matching_complete: bool,
    // Mode counts (after matching)
    create_count: usize,
    update_count: usize,
    unchanged_count: usize,
    error_count: usize,
}

impl State {
    fn new(file_path: PathBuf, sheet_name: String) -> Self {
        Self {
            current_environment: None,
            file_path,
            sheet_name,
            entities: Resource::NotAsked,
            detected_entity: None,
            manual_override: None,
            entity_selector: SelectField::new(),
            warnings: Vec::new(),
            warnings_list_state: ListState::default(),
            entity_data_loading: false,
            entity_data_loaded_count: 0,
            entity_data_total_count: 0,
            entity_data_cache: HashMap::new(),
            board_meeting_date_lookup: HashMap::new(),
            excel_processed: false,
            transformed_records: Vec::new(),
            ignored_rows: Vec::new(),
            total_rows: 0,
            rows_with_warnings: 0,
            existing_deadlines_loading: false,
            existing_deadlines_lookup: None,
            matching_complete: false,
            create_count: 0,
            update_count: 0,
            unchanged_count: 0,
            error_count: 0,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new(PathBuf::new(), String::new())
    }
}

#[derive(Clone)]
pub enum Msg {
    EnvironmentLoaded(Option<String>),
    EntitiesLoaded(Result<Vec<String>, String>),
    EntitySelectorEvent(SelectEvent),
    StartDataLoading,
    EntityDataLoaded(usize, Result<Vec<serde_json::Value>, String>),
    ExistingDeadlinesLoaded(Result<DeadlineLookupMap, String>),
    Back,
    Continue,
}

impl crate::tui::AppState for State {}

impl App for DeadlinesMappingApp {
    type State = State;
    type Msg = Msg;
    type InitParams = super::models::MappingParams;

    fn init(params: Self::InitParams) -> (State, Command<Msg>) {
        let mut state = State::new(
            params.file_path,
            params.sheet_name,
        );
        state.entities = Resource::Loading;

        // Load entities and current environment
        let cmd = Command::batch(vec![
            Command::perform(
                async {
                    let manager = crate::client_manager();
                    manager.get_current_environment_name().await
                        .ok()
                        .flatten()
                },
                Msg::EnvironmentLoaded
            ),
            Command::perform_parallel()
            .add_task(
                "Loading entities".to_string(),
                async move {
                    use crate::api::metadata::parse_entity_list;
                    let config = crate::global_config();
                    let manager = crate::client_manager();

                    // Get current environment
                    let environment_name = manager.get_current_environment_name().await
                        .map_err(|e| e.to_string())?
                        .ok_or_else(|| "No environment selected".to_string())?;

                    // Try cache first (24 hours)
                    match config.get_entity_cache(&environment_name, 24).await {
                        Ok(Some(cached)) => Ok::<Vec<String>, String>(cached),
                        _ => {
                            // Fetch from API
                            let client = manager
                                .get_current_client()
                                .await
                                .map_err(|e| e.to_string())?;
                            let metadata_xml = client.fetch_metadata().await.map_err(|e| e.to_string())?;
                            let entities = parse_entity_list(&metadata_xml).map_err(|e| e.to_string())?;

                            // Cache for future use
                            let _ = config.set_entity_cache(&environment_name, entities.clone()).await;

                            Ok(entities)
                        }
                    }
                }
            )
            .with_title("Loading entities")
            .on_complete(AppId::DeadlinesMapping)
            .cancellable(false)
            .build(move |_task_index, result| {
                let typed_result = result.downcast::<Result<Vec<String>, String>>()
                    .map(|boxed| *boxed)
                    .unwrap_or_else(|_| Err("Type mismatch in task result".to_string()));
                Msg::EntitiesLoaded(typed_result)
            })
        ]);

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::EnvironmentLoaded(env) => {
                state.current_environment = env;
                Command::None
            }
            Msg::EntitiesLoaded(result) => {
                state.entities = Resource::from_result(result);

                // Detect which entity type we should use
                if let Resource::Success(ref entities) = state.entities {
                    state.detected_entity = field_mappings::detect_deadline_entity(entities);

                    // If no detection, initialize selector with cgk/nrq options
                    if state.detected_entity.is_none() {
                        let options = vec!["cgk_deadline".to_string(), "nrq_deadline".to_string()];
                        state.entity_selector.state.update_option_count(options.len());
                        state.entity_selector.state.select(0);
                        state.entity_selector.set_value(Some(options[0].clone()));
                        state.manual_override = Some(options[0].clone());

                        return Command::set_focus(FocusId::new("entity-selector"));
                    }

                    // Entity detected - focus the load data button
                    return Command::set_focus(FocusId::new("load-data-button"));
                }

                Command::None
            }
            Msg::StartDataLoading => {
                let entity_type = state.detected_entity.clone()
                    .or_else(|| state.manual_override.clone());

                if let Some(entity_type) = entity_type {
                    // Start both entity data loading and existing deadlines loading in parallel
                    let entity_data_cmd = start_entity_data_loading(state, &entity_type);
                    let existing_deadlines_cmd = start_existing_deadlines_loading(state, &entity_type);
                    Command::batch(vec![entity_data_cmd, existing_deadlines_cmd])
                } else {
                    Command::None
                }
            }
            Msg::EntitySelectorEvent(event) => {
                let options = vec!["cgk_deadline".to_string(), "nrq_deadline".to_string()];
                let (cmd, _selection_event) = state.entity_selector.handle_event(event, &options);

                // If the selection changed, update manual override and reload entity data
                if let Some(new_selection) = state.entity_selector.value() {
                    let new_selection_str = new_selection.to_string();
                    if state.manual_override.as_ref() != Some(&new_selection_str) {
                        state.manual_override = Some(new_selection_str.clone());
                        return Command::batch(vec![
                            cmd,
                            start_entity_data_loading(state, &new_selection_str),
                        ]);
                    }
                }

                state.manual_override = state.entity_selector.value().map(|s| s.to_string());
                cmd
            }
            Msg::EntityDataLoaded(task_index, result) => {
                state.entity_data_loaded_count += 1;

                match result {
                    Ok(records) => {
                        log::debug!("Loaded {} records for task {}", records.len(), task_index);

                        // Store the entity data in cache
                        let entity_type = state.detected_entity.as_ref()
                            .or(state.manual_override.as_ref());
                        if let Some(entity_type) = entity_type {
                            let cache_entities = field_mappings::get_cache_entities(entity_type);
                            if task_index < cache_entities.len() {
                                let entity_name = cache_entities[task_index].clone();
                                state.entity_data_cache.insert(entity_name, records);
                            }
                        }
                    }
                    Err(err) => {
                        state.warnings.push(Warning(format!("Failed to load entity data: {}", err)));
                    }
                }

                // Check if all tasks completed
                if state.entity_data_loaded_count >= state.entity_data_total_count {
                    state.entity_data_loading = false;

                    // Process Excel file now that we have all entity data
                    if !state.excel_processed {
                        // Add temporary loading item to warnings list
                        state.warnings.clear();
                        state.warnings.push(Warning("Loading... Processing Excel file and validating data...".to_string()));
                        let cmd = process_excel_file(state);

                        // Try to run matching (if existing deadlines are already loaded)
                        try_run_matching(state);

                        return cmd;
                    }
                }

                Command::None
            }
            Msg::ExistingDeadlinesLoaded(result) => {
                state.existing_deadlines_loading = false;

                match result {
                    Ok(lookup_map) => {
                        log::info!(
                            "Loaded {} existing deadlines for matching",
                            lookup_map.len()
                        );
                        state.existing_deadlines_lookup = Some(lookup_map);
                    }
                    Err(err) => {
                        log::error!("Failed to load existing deadlines: {}", err);
                        state.warnings.push(Warning(format!(
                            "Failed to load existing deadlines (edit mode unavailable): {}",
                            err
                        )));
                        // Continue without existing deadlines - all records will be creates
                        state.existing_deadlines_lookup = None;
                    }
                }

                // Try to run matching (if Excel processing is already complete)
                try_run_matching(state);

                Command::None
            }
            Msg::Back => Command::batch(vec![
                Command::navigate_to(AppId::DeadlinesFileSelect),
                Command::quit_self(),
            ]),
            Msg::Continue => {
                Command::batch(vec![
                    Command::start_app(
                        AppId::DeadlinesInspection,
                        super::models::InspectionParams {
                            entity_type: state.detected_entity.clone()
                                .or_else(|| state.manual_override.clone())
                                .unwrap_or_default(),
                            transformed_records: state.transformed_records.clone(),
                        },
                    ),
                    Command::quit_self(),
                ])
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let content = match &state.entities {
            Resource::NotAsked => {
                col![Element::styled_text(Line::from(vec![Span::styled(
                    "Waiting...",
                    Style::default().fg(theme.text_tertiary)
                )]))
                .build()]
            }
            Resource::Loading => {
                // Loading screen is shown by the runtime
                col![]
            }
            Resource::Failure(err) => {
                col![
                    Element::styled_text(Line::from(vec![Span::styled(
                        "Error loading entities:",
                        Style::default().fg(theme.accent_error).bold()
                    )]))
                    .build(),
                    spacer!(),
                    Element::styled_text(Line::from(vec![Span::styled(
                        err.clone(),
                        Style::default().fg(theme.accent_error)
                    )]))
                    .build(),
                    spacer!(),
                    Element::button("back-button", "Back")
                        .on_press(Msg::Back)
                        .build(),
                ]
            }
            Resource::Success(entities) => {
                use crate::tui::element::ColumnBuilder;
                let mut builder = ColumnBuilder::new();

                // Top section: info lines
                if let Some(ref env) = state.current_environment {
                    builder = builder.add(Element::styled_text(Line::from(vec![
                        Span::styled("Environment: ", Style::default().fg(theme.text_tertiary)),
                        Span::styled(
                            env.clone(),
                            Style::default().fg(theme.accent_primary)
                        ),
                    ]))
                    .build(), Length(1));
                }

                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled("File: ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(
                        state.file_path.file_name().unwrap().to_string_lossy().to_string(),
                        Style::default().fg(theme.text_primary)
                    ),
                ]))
                .build(), Length(1));

                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled("Sheet: ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(state.sheet_name.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(), Length(1));

                builder = builder.add(spacer!(), Length(1));

                // Mapping info based on detected entity or manual override
                let selected_entity = state.detected_entity.as_ref()
                    .or(state.manual_override.as_ref());

                match selected_entity {
                    Some(entity_type) => {
                        let mapping_count = field_mappings::get_mappings_for_entity(entity_type).len();

                        if state.detected_entity.is_some() {
                            builder = builder.add(Element::styled_text(Line::from(vec![
                                Span::styled("✓ ", Style::default().fg(theme.accent_success)),
                                Span::styled(
                                    format!("Detected entity: "),
                                    Style::default().fg(theme.text_tertiary)
                                ),
                                Span::styled(
                                    entity_type.clone(),
                                    Style::default().fg(theme.accent_primary).bold()
                                ),
                            ]))
                            .build(), Length(1));
                        } else {
                            builder = builder.add(Element::styled_text(Line::from(vec![
                                Span::styled("⚠ ", Style::default().fg(theme.accent_warning)),
                                Span::styled(
                                    "Could not auto-detect entity. Using manual selection: ",
                                    Style::default().fg(theme.accent_warning)
                                ),
                                Span::styled(
                                    entity_type.clone(),
                                    Style::default().fg(theme.accent_primary).bold()
                                ),
                            ]))
                            .build(), Length(1));
                        }

                        builder = builder.add(Element::styled_text(Line::from(vec![
                            Span::styled(
                                format!("Will use {} static field mappings", mapping_count),
                                Style::default().fg(theme.text_primary)
                            ),
                        ]))
                        .build(), Length(1));
                        builder = builder.add(Element::styled_text(Line::from(vec![
                            Span::styled(
                                "Checkbox columns will be detected dynamically from entity metadata",
                                Style::default().fg(theme.text_tertiary).italic()
                            ),
                        ]))
                        .build(), Length(1));
                    }
                    None => {
                        builder = builder.add(Element::styled_text(Line::from(vec![
                            Span::styled("⚠ ", Style::default().fg(theme.accent_warning)),
                            Span::styled(
                                "Could not detect cgk_deadline or nrq_deadline entity",
                                Style::default().fg(theme.accent_warning)
                            ),
                        ]))
                        .build(), Length(1));
                        builder = builder.add(spacer!(), Length(1));

                        // Add manual entity selector
                        let options = vec!["cgk_deadline".to_string(), "nrq_deadline".to_string()];
                        let selector = Element::select("entity-selector", options, &mut state.entity_selector.state)
                            .on_event(Msg::EntitySelectorEvent)
                            .build();

                        let selector_panel = Element::panel(selector)
                            .title("Select Entity Type")
                            .build();

                        builder = builder.add(selector_panel, Length(5));
                    }
                }

                builder = builder.add(spacer!(), Length(1));

                // Show warnings list if data is loaded and processed, or instruction text
                if state.excel_processed {
                    // Show warnings list after processing
                    let warnings_list = Element::list("warnings-list", &state.warnings, &state.warnings_list_state, theme)
                        .build();

                    let panel_title = if state.ignored_rows.is_empty() {
                        "Warnings & Notes".to_string()
                    } else {
                        format!("Warnings & Notes ({} rows ignored)", state.ignored_rows.len())
                    };

                    let warnings_panel = Element::panel(warnings_list)
                        .title(&panel_title)
                        .build();

                    builder = builder.add(warnings_panel, Fill(1));
                } else if state.entity_data_loading {
                    // Show loading status
                    builder = builder.add(Element::styled_text(Line::from(vec![Span::styled(
                        format!("Loading entity data... ({}/{})", state.entity_data_loaded_count, state.entity_data_total_count),
                        Style::default().fg(theme.accent_warning)
                    )]))
                    .build(), Length(1));
                    builder = builder.add(spacer!(), Fill(1));
                } else {
                    // Show instruction to load data
                    builder = builder.add(Element::styled_text(Line::from(vec![Span::styled(
                        "Click 'Load Data' to fetch entity records for validation",
                        Style::default().fg(theme.text_tertiary).italic()
                    )]))
                    .build(), Length(1));
                    builder = builder.add(spacer!(), Fill(1));
                }

                // Bottom section: buttons
                let right_button = if state.matching_complete {
                    // Show Continue button after matching is complete
                    Element::button("continue-button", "Continue")
                        .on_press(Msg::Continue)
                        .build()
                } else if state.excel_processed || state.existing_deadlines_loading {
                    // Show loading indicator while processing
                    Element::button("loading-button", "Loading...")
                        .build()
                } else {
                    // Show Load Data button before data is loaded
                    Element::button("load-data-button", "Load Data")
                        .on_press(Msg::StartDataLoading)
                        .build()
                };

                builder = builder.add(crate::row![
                    Element::button("back-button", "Back")
                        .on_press(Msg::Back)
                        .build(),
                    spacer!(),
                    right_button,
                ], Length(3));

                builder.build()
            }
        };

        let outer_panel = Element::panel(content)
            .title("Deadlines - Field Mapping Configuration")
            .build();

        LayeredView::new(outer_panel)
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        vec![]
    }

    fn title() -> &'static str {
        "Deadlines - Mapping"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        state.current_environment.as_ref().map(|env| {
        let theme = &crate::global_runtime_config().theme;
            Line::from(vec![
                Span::styled("Environment: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(
                    env.clone(),
                    Style::default().fg(theme.accent_primary),
                ),
            ])
        })
    }
}
