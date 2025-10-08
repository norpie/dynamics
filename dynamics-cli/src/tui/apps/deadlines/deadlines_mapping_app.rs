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
use super::models::TransformedDeadline;

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
fn get_checkbox_relationship_names(entity_type: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    if entity_type == "cgk_deadline" {
        map.insert("cgk_support".to_string(), "cgk_deadline_cgk_support".to_string());
        map.insert("cgk_category".to_string(), "cgk_deadline_cgk_category".to_string());
        map.insert("cgk_length".to_string(), "cgk_deadline_cgk_length".to_string());
        map.insert("cgk_flemishshare".to_string(), "cgk_deadline_cgk_flemishshare".to_string());
    } else if entity_type == "nrq_deadline" {
        map.insert("nrq_support".to_string(), "nrq_deadline_nrq_support".to_string());
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
    if let Some(records) = cache.get(entity_name) {
        for record in records {
            if let Some(name) = extract_name_from_record_with_entity(record, Some(entity_name)) {
                if name.trim().to_lowercase() == header.trim().to_lowercase() {
                    return extract_id_from_record(record, entity_name);
                }
            }
        }
    }
    None
}

/// Resolve checked checkbox values to their IDs and relationship names
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
    let checkbox_entities = vec![
        format!("{}support", entity_prefix),
        format!("{}category", entity_prefix),
        if entity_prefix == "cgk_" { "cgk_length".to_string() } else { "nrq_subcategory".to_string() },
        format!("{}flemishshare", entity_prefix),
    ];

    // Process columns after RvB
    for (col_idx, header) in headers.iter().enumerate().skip(rvb_idx + 1) {
        // Skip empty headers and OPM column
        if header.is_empty() || header.to_uppercase() == "OPM" {
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

    // Fetch all records for this entity using query builder
    let query = crate::api::QueryBuilder::new(&plural_name).build();
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
        "nrq_boardmeeting" // Separate entity for NRQ (logical name, not full entity name)
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
                        vec!["nrq_boardmeetingid", "nrq_boardofdirectorsmeetingid"]
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

/// Process Excel file: find header row, identify checkbox columns, transform rows
fn process_excel_file(state: &mut State) -> Command<Msg> {
    state.excel_processed = true;
    state.warnings.clear();
    state.transformed_records.clear();

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

    // Find "Raad van Bestuur" column index
    let rvb_idx = headers.iter().position(|h| h.to_lowercase().contains("raad") && h.to_lowercase().contains("bestuur"));

    if rvb_idx.is_none() {
        state.warnings.push(Warning("Could not find 'Raad van Bestuur' column".to_string()));
        return Command::None;
    }

    let rvb_idx = rvb_idx.unwrap();
    log::debug!("Found 'Raad van Bestuur' at column index {}", rvb_idx);

    // All columns after RvB are checkbox columns, except "OPM"
    let checkbox_columns: Vec<String> = headers.iter()
        .skip(rvb_idx + 1)
        .filter(|h| !h.is_empty() && h.to_uppercase() != "OPM")
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

    // Now process data rows and build transformed records
    let data_rows = range.rows().skip(header_row_idx + 1);

    for (row_num, row) in data_rows.enumerate() {
        let excel_row_number = header_row_idx + 2 + row_num; // +2 because: +1 for header, +1 for 1-based indexing
        let transformed = process_row(state, &headers, row, excel_row_number, &entity_type_owned);

        // Add row-level warnings to global warnings list
        for warning in &transformed.warnings {
            state.warnings.push(Warning(format!("Row {}: {}", excel_row_number, warning)));
        }

        state.transformed_records.push(transformed);
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

    // Get field mappings for this entity type
    let mappings = field_mappings::get_mappings_for_entity(entity_type);

    // Determine field name prefixes
    let date_field = if entity_type == "cgk_deadline" { "cgk_date" } else { "nrq_date" };
    let time_field = if entity_type == "cgk_deadline" { "cgk_date" } else { "nrq_time" };
    let commission_date_field = if entity_type == "cgk_deadline" { "cgk_datumcommissievergadering" } else { "nrq_commissiondate" };

    // Process each mapped field
    for mapping in mappings {
        // Find the column index for this field
        if let Some(col_idx) = headers.iter().position(|h| h == &mapping.excel_column) {
            if let Some(cell) = row.get(col_idx) {
                let cell_value = cell.to_string();

                // Skip empty cells
                if cell_value.trim().is_empty() {
                    continue;
                }

                match &mapping.field_type {
                    field_mappings::FieldType::Lookup { target_entity } => {
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
                        // Parse time (to be combined with deadline date)
                        match parse_time(&cell_value) {
                            Ok(time) => {
                                transformed.deadline_time = Some(time);
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
                        // Direct field - copy value as-is
                        transformed.direct_fields.insert(
                            mapping.dynamics_field.clone(),
                            cell_value.clone()
                        );
                    }
                    field_mappings::FieldType::Checkbox => {
                        // Checkboxes are handled separately below
                    }
                }
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

    for (index, entity_name) in cache_entities.iter().enumerate() {
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

pub struct DeadlinesMappingApp;

// Wrapper type for warnings to implement ListItem
#[derive(Clone)]
struct Warning(String);

impl crate::tui::widgets::ListItem for Warning {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_hovered: bool) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (theme.lavender, Some(Style::default().bg(theme.surface0)))
        } else {
            (theme.text, None)
        };

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(theme.yellow)),
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
    total_rows: usize,
    rows_with_warnings: usize,
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
            total_rows: 0,
            rows_with_warnings: 0,
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
                    start_entity_data_loading(state, &entity_type)
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
                        return process_excel_file(state);
                    }
                }

                Command::None
            }
            Msg::Back => Command::navigate_to(AppId::DeadlinesFileSelect),
            Msg::Continue => {
                Command::start_app(
                    AppId::DeadlinesInspection,
                    super::models::InspectionParams {
                        entity_type: state.detected_entity.clone()
                            .or_else(|| state.manual_override.clone())
                            .unwrap_or_default(),
                        transformed_records: state.transformed_records.clone(),
                    },
                )
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let content = match &state.entities {
            Resource::NotAsked => {
                col![Element::styled_text(Line::from(vec![Span::styled(
                    "Waiting...",
                    Style::default().fg(theme.subtext0)
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
                        Style::default().fg(theme.red).bold()
                    )]))
                    .build(),
                    spacer!(),
                    Element::styled_text(Line::from(vec![Span::styled(
                        err.clone(),
                        Style::default().fg(theme.red)
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
                        Span::styled("Environment: ", Style::default().fg(theme.subtext0)),
                        Span::styled(
                            env.clone(),
                            Style::default().fg(theme.lavender)
                        ),
                    ]))
                    .build(), Length(1));
                }

                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled("File: ", Style::default().fg(theme.subtext0)),
                    Span::styled(
                        state.file_path.file_name().unwrap().to_string_lossy().to_string(),
                        Style::default().fg(theme.text)
                    ),
                ]))
                .build(), Length(1));

                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled("Sheet: ", Style::default().fg(theme.subtext0)),
                    Span::styled(state.sheet_name.clone(), Style::default().fg(theme.text)),
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
                                Span::styled("✓ ", Style::default().fg(theme.green)),
                                Span::styled(
                                    format!("Detected entity: "),
                                    Style::default().fg(theme.subtext0)
                                ),
                                Span::styled(
                                    entity_type.clone(),
                                    Style::default().fg(theme.lavender).bold()
                                ),
                            ]))
                            .build(), Length(1));
                        } else {
                            builder = builder.add(Element::styled_text(Line::from(vec![
                                Span::styled("⚠ ", Style::default().fg(theme.yellow)),
                                Span::styled(
                                    "Could not auto-detect entity. Using manual selection: ",
                                    Style::default().fg(theme.yellow)
                                ),
                                Span::styled(
                                    entity_type.clone(),
                                    Style::default().fg(theme.lavender).bold()
                                ),
                            ]))
                            .build(), Length(1));
                        }

                        builder = builder.add(Element::styled_text(Line::from(vec![
                            Span::styled(
                                format!("Will use {} static field mappings", mapping_count),
                                Style::default().fg(theme.text)
                            ),
                        ]))
                        .build(), Length(1));
                        builder = builder.add(Element::styled_text(Line::from(vec![
                            Span::styled(
                                "Checkbox columns will be detected dynamically from entity metadata",
                                Style::default().fg(theme.subtext0).italic()
                            ),
                        ]))
                        .build(), Length(1));
                    }
                    None => {
                        builder = builder.add(Element::styled_text(Line::from(vec![
                            Span::styled("⚠ ", Style::default().fg(theme.yellow)),
                            Span::styled(
                                "Could not detect cgk_deadline or nrq_deadline entity",
                                Style::default().fg(theme.yellow)
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

                    let warnings_panel = Element::panel(warnings_list)
                        .title("Mapping Warnings")
                        .build();

                    builder = builder.add(warnings_panel, Fill(1));
                } else if state.entity_data_loading {
                    // Show loading status
                    builder = builder.add(Element::styled_text(Line::from(vec![Span::styled(
                        format!("Loading entity data... ({}/{})", state.entity_data_loaded_count, state.entity_data_total_count),
                        Style::default().fg(theme.yellow)
                    )]))
                    .build(), Length(1));
                    builder = builder.add(spacer!(), Fill(1));
                } else {
                    // Show instruction to load data
                    builder = builder.add(Element::styled_text(Line::from(vec![Span::styled(
                        "Click 'Load Data' to fetch entity records for validation",
                        Style::default().fg(theme.subtext0).italic()
                    )]))
                    .build(), Length(1));
                    builder = builder.add(spacer!(), Fill(1));
                }

                // Bottom section: buttons
                let right_button = if state.excel_processed {
                    // Show Continue button after data is processed
                    Element::button("continue-button", "Continue")
                        .on_press(Msg::Continue)
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
                Span::styled("Environment: ", Style::default().fg(theme.subtext0)),
                Span::styled(
                    env.clone(),
                    Style::default().fg(theme.lavender),
                ),
            ])
        })
    }
}
