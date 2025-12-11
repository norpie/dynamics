//! Transfer Preview app - displays resolved records after transform

use std::collections::HashMap;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::config::repository::transfer::get_transfer_config;
use crate::transfer::{RecordAction, ResolvedTransfer, TransferConfig, TransformEngine};
use crate::tui::resource::Resource;
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use super::state::{Msg, PreviewParams, RecordDetailState, RecordFilter, State};
use super::view;

/// Transfer Preview App - shows resolved records before execution
pub struct TransferPreviewApp;

impl crate::tui::AppState for State {}

impl App for TransferPreviewApp {
    type State = State;
    type Msg = Msg;
    type InitParams = PreviewParams;

    fn init(params: PreviewParams) -> (State, Command<Msg>) {
        let state = State {
            config_name: params.config_name.clone(),
            source_env: params.source_env.clone(),
            target_env: params.target_env.clone(),
            resolved: Resource::Loading,
            ..Default::default()
        };

        // First load config to know which entities to fetch
        let cmd = Command::perform(
            load_config(params.config_name),
            Msg::ConfigLoaded,
        );

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            // Data loading - Step 1: Config loaded, now fetch records
            Msg::ConfigLoaded(result) => {
                match result {
                    Ok(config) => {
                        // Build parallel fetch tasks for loading screen
                        let mut builder = Command::perform_parallel()
                            .with_title("Fetching Records");

                        let num_entities = config.entity_mappings.len();

                        // Add source fetch tasks with progress reporting
                        for mapping in &config.entity_mappings {
                            let entity = mapping.source_entity.clone();
                            let env = config.source_env.clone();

                            // Collect source fields from transforms + primary key
                            let mut source_fields: Vec<String> = mapping
                                .field_mappings
                                .iter()
                                .flat_map(|fm| fm.transform.source_fields())
                                .map(|s| s.to_string())
                                .collect();
                            source_fields.push(format!("{}id", entity)); // Primary key
                            source_fields.sort();
                            source_fields.dedup();

                            // Collect expand specifications for lookup traversals
                            // Group by lookup field to combine selects: accountid($select=name,email)
                            let mut expand_map: std::collections::HashMap<String, Vec<String>> =
                                std::collections::HashMap::new();
                            for fm in &mapping.field_mappings {
                                for (lookup_field, target_field) in fm.transform.expand_specs() {
                                    expand_map
                                        .entry(lookup_field.to_string())
                                        .or_default()
                                        .push(target_field.to_string());
                                }
                            }
                            // Build expand strings: "lookupfield($select=field1,field2)"
                            let expands: Vec<String> = expand_map
                                .into_iter()
                                .map(|(lookup, mut fields)| {
                                    fields.sort();
                                    fields.dedup();
                                    format!("{}($select={})", lookup, fields.join(","))
                                })
                                .collect();

                            log::info!(
                                "[{}] Source fetch will select {} fields, expand {} lookups",
                                entity,
                                source_fields.len(),
                                expands.len()
                            );
                            if !expands.is_empty() {
                                log::info!("[{}] Expands: {:?}", entity, expands);
                            }

                            builder = builder.add_task_with_progress(
                                format!("Source: {}", entity),
                                move |progress| fetch_entity_records(env, entity, true, source_fields, expands, Some(progress)),
                            );
                        }

                        // Add target fetch tasks with progress reporting
                        for mapping in &config.entity_mappings {
                            let entity = mapping.target_entity.clone();
                            let env = config.target_env.clone();

                            // Collect target fields + primary key
                            let mut target_fields: Vec<String> = mapping
                                .field_mappings
                                .iter()
                                .map(|fm| fm.target_field.clone())
                                .collect();
                            target_fields.push(format!("{}id", entity)); // Primary key
                            target_fields.sort();
                            target_fields.dedup();

                            log::info!("[{}] Target fetch will select {} fields", entity, target_fields.len());

                            // Target fetch doesn't need expands - we compare final values
                            let no_expands: Vec<String> = vec![];

                            builder = builder.add_task_with_progress(
                                format!("Target: {}", entity),
                                move |progress| fetch_entity_records(env, entity, false, target_fields, no_expands, Some(progress)),
                            );
                        }

                        // Track how many fetches we're waiting for (source + target for each entity)
                        state.pending_fetches = num_entities * 2;
                        state.config = Some(config);

                        builder
                            .on_complete(AppId::TransferPreview)
                            .build(|_task_idx, result| {
                                let data = result
                                    .downcast::<Result<(String, bool, Vec<serde_json::Value>), String>>()
                                    .unwrap();
                                Msg::FetchResult(*data)
                            })
                    }
                    Err(e) => {
                        state.resolved = Resource::Failure(e);
                        Command::None
                    }
                }
            }

            // Data loading - Step 2: Each fetch result comes in individually
            Msg::FetchResult(result) => {
                match result {
                    Ok((entity_name, is_source, records)) => {
                        if is_source {
                            state.source_data.insert(entity_name, records);
                        } else {
                            state.target_data.insert(entity_name, records);
                        }

                        state.pending_fetches = state.pending_fetches.saturating_sub(1);

                        // Just accumulate data - transform runs after loading screen returns
                        if state.pending_fetches == 0 {
                            log::info!("All fetches complete, data ready for transform");
                            // Mark as loading - transform will run when we become active again
                            state.resolved = Resource::Loading;
                        }
                    }
                    Err(e) => {
                        state.resolved = Resource::Failure(e);
                        state.pending_fetches = 0;
                    }
                }
                Command::None
            }

            // Data loading - Step 3: Run transform after returning from loading screen
            Msg::RunTransform => {
                if let Some(config) = state.config.take() {
                    // Build primary keys map for both source and target entities
                    let mut primary_keys: HashMap<String, String> = HashMap::new();
                    for m in &config.entity_mappings {
                        primary_keys.insert(m.source_entity.clone(), format!("{}id", m.source_entity));
                        primary_keys.insert(m.target_entity.clone(), format!("{}id", m.target_entity));
                    }

                    // Run transform
                    let resolved = TransformEngine::transform_all(
                        &config,
                        &state.source_data,
                        &state.target_data,
                        &primary_keys,
                    );

                    log::info!(
                        "Transform complete: {} records ({} create, {} update, {} nochange, {} skip, {} error)",
                        resolved.total_records(),
                        resolved.create_count(),
                        resolved.update_count(),
                        resolved.nochange_count(),
                        resolved.skip_count(),
                        resolved.error_count()
                    );

                    // Clear accumulated data
                    state.source_data.clear();
                    state.target_data.clear();

                    state.resolved = Resource::Success(resolved);
                }
                Command::None
            }

            Msg::ResolvedLoaded(result) => {
                state.resolved = match result {
                    Ok(resolved) => Resource::Success(resolved),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            // Navigation within table
            Msg::ListEvent(event) => {
                // Count filtered records for proper navigation bounds
                let item_count = if let Resource::Success(resolved) = &state.resolved {
                    let query = state.search_field.value().to_lowercase();
                    resolved.entities.get(state.current_entity_idx)
                        .map(|e| {
                            // Count only records matching current filter and search
                            e.records.iter()
                                .filter(|r| state.filter.matches(r.action))
                                .filter(|r| {
                                    if query.is_empty() {
                                        return true;
                                    }
                                    if r.source_id.to_string().to_lowercase().contains(&query) {
                                        return true;
                                    }
                                    r.fields.values().any(|v| {
                                        format!("{:?}", v).to_lowercase().contains(&query)
                                    })
                                })
                                .count()
                        })
                        .unwrap_or(0)
                } else {
                    0
                };
                state.list_state.handle_event(event, item_count, state.viewport_height);
                Command::None
            }

            // Viewport height changed (for virtual scrolling)
            Msg::ViewportHeightChanged(height) => {
                state.viewport_height = height;
                Command::None
            }

            Msg::NextEntity => {
                if let Resource::Success(resolved) = &state.resolved {
                    if state.current_entity_idx + 1 < resolved.entities.len() {
                        state.current_entity_idx += 1;
                        state.list_state = crate::tui::widgets::ListState::with_selection();
                    }
                }
                Command::None
            }

            Msg::PrevEntity => {
                if state.current_entity_idx > 0 {
                    state.current_entity_idx -= 1;
                    state.list_state = crate::tui::widgets::ListState::with_selection();
                }
                Command::None
            }

            Msg::SelectEntity(idx) => {
                if let Resource::Success(resolved) = &state.resolved {
                    if idx < resolved.entities.len() {
                        state.current_entity_idx = idx;
                        state.list_state = crate::tui::widgets::ListState::with_selection();
                    }
                }
                Command::None
            }

            // Filtering
            Msg::SetFilter(filter) => {
                state.filter = filter;
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            Msg::CycleFilter => {
                state.filter = state.filter.next();
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            Msg::SearchChanged(event) => {
                state.search_field.handle_event(event, None);
                // Reset list selection when search changes
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            // Record actions
            Msg::ToggleSkip => {
                // TODO (Chunk 8): Implement skip toggle
                Command::None
            }

            Msg::ViewDetails => {
                if let Some(idx) = state.list_state.selected() {
                    if let Resource::Success(resolved) = &state.resolved {
                        if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                            // Get filtered records to find the actual record
                            let filtered: Vec<_> = entity.records.iter()
                                .filter(|r| state.filter.matches(r.action))
                                .filter(|r| {
                                    let query = state.search_field.value().to_lowercase();
                                    if query.is_empty() { return true; }
                                    if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                                    r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                })
                                .collect();

                            if let Some(record) = filtered.get(idx) {
                                state.record_detail_state = Some(RecordDetailState::new(
                                    idx,
                                    record.action,
                                    &entity.field_names,
                                    &record.fields,
                                ));
                                state.active_modal = Some(super::state::PreviewModal::RecordDetails {
                                    record_idx: idx,
                                });
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::EditRecord => {
                if let Some(idx) = state.list_state.selected() {
                    if let Resource::Success(resolved) = &state.resolved {
                        if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                            let filtered: Vec<_> = entity.records.iter()
                                .filter(|r| state.filter.matches(r.action))
                                .filter(|r| {
                                    let query = state.search_field.value().to_lowercase();
                                    if query.is_empty() { return true; }
                                    if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                                    r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                })
                                .collect();

                            if let Some(record) = filtered.get(idx) {
                                let mut detail_state = RecordDetailState::new(
                                    idx,
                                    record.action,
                                    &entity.field_names,
                                    &record.fields,
                                );
                                detail_state.editing = true; // Start in edit mode
                                state.record_detail_state = Some(detail_state);
                                state.active_modal = Some(super::state::PreviewModal::RecordDetails {
                                    record_idx: idx,
                                });
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::SaveRecord => {
                state.active_modal = None;
                state.record_detail_state = None;
                Command::None
            }

            // Record details modal handlers
            Msg::ToggleEditMode => {
                if let Some(ref mut detail) = state.record_detail_state {
                    detail.editing = !detail.editing;
                }
                Command::None
            }

            Msg::RecordDetailActionChanged(action) => {
                if let Some(ref mut detail) = state.record_detail_state {
                    detail.current_action = action;
                }
                Command::None
            }

            Msg::RecordDetailFieldNavigate(key) => {
                if let Some(ref mut detail) = state.record_detail_state {
                    // Only navigate when not actively editing a field
                    if !detail.editing_field {
                        let field_count = detail.fields.len();
                        if field_count > 0 {
                            match key {
                                crossterm::event::KeyCode::Up => {
                                    if detail.focused_field_idx > 0 {
                                        detail.focused_field_idx -= 1;
                                    }
                                }
                                crossterm::event::KeyCode::Down => {
                                    if detail.focused_field_idx + 1 < field_count {
                                        detail.focused_field_idx += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::StartFieldEdit => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing && !detail.editing_field {
                        detail.editing_field = true;
                    }
                }
                Command::None
            }

            Msg::FocusedFieldInput(event) => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing_field {
                        if let Some(field) = detail.fields.get_mut(detail.focused_field_idx) {
                            field.input.handle_event(event, None);
                            field.update_dirty();
                        }
                    }
                }
                Command::None
            }

            Msg::FinishFieldEdit => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing_field {
                        detail.editing_field = false;
                        // Move to next field
                        if detail.focused_field_idx + 1 < detail.fields.len() {
                            detail.focused_field_idx += 1;
                        }
                    }
                }
                Command::None
            }

            Msg::CancelFieldEdit => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing_field {
                        // Reset field to original value
                        if let Some(field) = detail.fields.get_mut(detail.focused_field_idx) {
                            field.reset();
                        }
                        detail.editing_field = false;
                    }
                }
                Command::None
            }

            Msg::SaveRecordEdits => {
                // Apply changes to the resolved record
                if let Some(ref detail) = state.record_detail_state {
                    if let Resource::Success(ref mut resolved) = state.resolved {
                        if let Some(entity) = resolved.entities.get_mut(state.current_entity_idx) {
                            // Find the actual record by filtering the same way
                            let filter = state.filter;
                            let query = state.search_field.value().to_lowercase();
                            let record_idx = detail.record_idx;

                            // First pass: find the record's source_id
                            let mut match_idx = 0;
                            let mut target_source_id = None;
                            for record in &entity.records {
                                if !filter.matches(record.action) {
                                    continue;
                                }
                                let matches_search = if query.is_empty() {
                                    true
                                } else if record.source_id.to_string().to_lowercase().contains(&query) {
                                    true
                                } else {
                                    record.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                };
                                if !matches_search {
                                    continue;
                                }

                                if match_idx == record_idx {
                                    target_source_id = Some(record.source_id);
                                    break;
                                }
                                match_idx += 1;
                            }

                            // Second pass: apply changes to the found record
                            if let Some(source_id) = target_source_id {
                                // Find and update the record
                                if let Some(record) = entity.records.iter_mut().find(|r| r.source_id == source_id) {
                                    // Apply action change
                                    if detail.current_action != detail.original_action {
                                        record.action = detail.current_action;
                                        if detail.current_action != RecordAction::Error {
                                            record.error = None;
                                        }
                                    }

                                    // Apply field changes
                                    for field_state in &detail.fields {
                                        if field_state.is_dirty {
                                            let new_value = field_state.parse_value();
                                            record.fields.insert(field_state.field_name.clone(), new_value);
                                        }
                                    }
                                }

                                // Mark as dirty in entity
                                entity.mark_dirty(source_id);
                            }
                        }
                    }
                }

                state.active_modal = None;
                state.record_detail_state = None;
                Command::None
            }

            Msg::CancelRecordEdits => {
                // Just close without saving
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing {
                        // If in edit mode, switch back to view mode and reset
                        detail.reset_all();
                        detail.editing = false;
                    } else {
                        // If in view mode, close the modal
                        state.active_modal = None;
                        state.record_detail_state = None;
                    }
                }
                Command::None
            }

            // Bulk actions
            Msg::OpenBulkActions => {
                state.active_modal = Some(super::state::PreviewModal::BulkActions);
                Command::None
            }

            Msg::ApplyBulkAction(_action) => {
                // TODO (Chunk 8): Implement bulk action
                state.active_modal = None;
                Command::None
            }

            // Excel
            Msg::ExportExcel => {
                // TODO (Chunk 9): Implement Excel export
                Command::None
            }

            Msg::ImportExcel => {
                // TODO (Chunk 10): Implement Excel import
                Command::None
            }

            Msg::ExportCompleted(result) => {
                match result {
                    Ok(path) => log::info!("Exported to {}", path),
                    Err(e) => log::error!("Export failed: {}", e),
                }
                Command::None
            }

            Msg::ImportCompleted(result) => {
                match result {
                    Ok(resolved) => {
                        state.resolved = Resource::Success(resolved);
                    }
                    Err(e) => log::error!("Import failed: {}", e),
                }
                state.active_modal = None;
                Command::None
            }

            // Refresh
            Msg::Refresh => {
                // TODO (Chunk 11): Re-run transform
                Command::None
            }

            // Modal
            Msg::CloseModal => {
                state.active_modal = None;
                state.record_detail_state = None;
                Command::None
            }

            // Navigation
            Msg::Back => {
                Command::navigate_to(AppId::TransferMappingEditor)
            }

            Msg::GoToExecute => {
                // TODO (Chunk 12): Navigate to execute app
                log::info!("Would navigate to execute");
                Command::None
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        view::render(state, theme)
    }

    fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
        view::subscriptions(state)
    }

    fn title() -> &'static str {
        "Transfer Preview"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;

        match &state.resolved {
            Resource::NotAsked => None,
            Resource::Loading => Some(Line::from(vec![
                Span::styled("Loading...", Style::default().fg(theme.text_secondary)),
            ])),
            Resource::Failure(err) => Some(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(theme.accent_error)),
                Span::styled(err.clone(), Style::default().fg(theme.text_primary)),
            ])),
            Resource::Success(resolved) => {
                if resolved.entities.is_empty() {
                    return Some(Line::from("No entities"));
                }

                let entity = &resolved.entities[state.current_entity_idx];
                let filtered_count = entity.records.iter()
                    .filter(|r| state.filter.matches(r.action))
                    .filter(|r| {
                        let query = state.search_field.value().to_lowercase();
                        if query.is_empty() {
                            return true;
                        }
                        // Search in source_id and field values
                        if r.source_id.to_string().to_lowercase().contains(&query) {
                            return true;
                        }
                        r.fields.values().any(|v| {
                            format!("{:?}", v).to_lowercase().contains(&query)
                        })
                    })
                    .count();

                Some(Line::from(vec![
                    Span::styled(entity.entity_name.clone(), Style::default().fg(theme.accent_primary)),
                    Span::styled(
                        format!(" ({}/{})", state.current_entity_idx + 1, resolved.entities.len()),
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::raw(" | "),
                    Span::styled(format!("{}", entity.create_count()), Style::default().fg(theme.accent_success)),
                    Span::styled(" create".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.update_count()), Style::default().fg(theme.accent_secondary)),
                    Span::styled(" update".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.nochange_count()), Style::default().fg(theme.text_tertiary)),
                    Span::styled(" unchanged".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.skip_count()), Style::default().fg(theme.accent_warning)),
                    Span::styled(" skip".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.error_count()), Style::default().fg(theme.accent_error)),
                    Span::styled(" error".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" | "),
                    Span::styled(state.filter.display_name().to_string(), Style::default().fg(theme.accent_primary)),
                    Span::styled(format!(" ({})", filtered_count), Style::default().fg(theme.text_secondary)),
                ]))
            }
        }
    }

    fn on_resume(state: &mut State) -> Command<Msg> {
        // If we have data but haven't transformed yet, run the transform now
        if matches!(state.resolved, Resource::Loading)
            && state.config.is_some()
            && !state.source_data.is_empty()
        {
            log::info!("TransferPreviewApp resuming with data ready - triggering transform");
            // Use a message to run transform so UI can update first
            Command::perform(async { () }, |_| Msg::RunTransform)
        } else {
            Command::None
        }
    }
}

// =============================================================================
// Async helper functions
// =============================================================================

/// Load transfer config from database
async fn load_config(config_name: String) -> Result<TransferConfig, String> {
    let pool = &crate::global_config().pool;
    get_transfer_config(pool, &config_name)
        .await
        .map_err(|e| format!("Failed to load config: {}", e))?
        .ok_or_else(|| format!("Config '{}' not found", config_name))
}

/// Fetch all records for an entity from an environment
/// Returns (entity_name, is_source, records)
async fn fetch_entity_records(
    env_name: String,
    entity_name: String,
    is_source: bool,
    fields: Vec<String>,  // Fields to select (for performance)
    expands: Vec<String>, // Expand clauses for lookup traversals
    progress: Option<crate::tui::command::ProgressSender>,
) -> Result<(String, bool, Vec<serde_json::Value>), String> {
    use crate::api::pluralization::pluralize_entity_name;
    use crate::api::query::QueryBuilder;

    let manager = crate::client_manager();
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    let entity_set = pluralize_entity_name(&entity_name);

    // First: get real count via FetchXML aggregate (OData $count caps at 5000)
    let count_fetchxml = format!(
        r#"<fetch aggregate="true"><entity name="{}"><attribute name="{}id" aggregate="count" alias="total"/><filter><condition attribute="statecode" operator="eq" value="0"/></filter></entity></fetch>"#,
        entity_name, entity_name
    );

    let total_count: Option<u64> = match client.execute_fetchxml(&entity_name, &count_fetchxml).await {
        Ok(result) => {
            result.get("value")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|obj| obj.get("total"))
                .and_then(|t| t.as_u64())
        }
        Err(e) => {
            log::warn!("[{}] Count query failed, progress will show records only: {}", entity_name, e);
            None
        }
    };
    log::info!("[{}] Total count: {:?}", entity_name, total_count);

    // Report initial progress
    if let Some(ref tx) = progress {
        let msg = match total_count {
            Some(total) => format!("0/{}", total),
            None => "Starting...".to_string(),
        };
        let _ = tx.send(msg);
    }

    // Fetch data using @odata.nextLink pagination (Dynamics doesn't support $skip)
    let mut all_records = Vec::new();
    let mut page = 0;

    log::info!("[{}] 🚀 Starting data fetch...", entity_name);
    let fetch_start = std::time::Instant::now();

    // Build initial query (no $top - let API return default 5000 with nextLink)
    let mut builder = QueryBuilder::new(&entity_set).active_only();
    if !fields.is_empty() {
        let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
        builder = builder.select(&field_refs);
    }
    // Add $expand for lookup traversals
    if !expands.is_empty() {
        let expand_refs: Vec<&str> = expands.iter().map(|s| s.as_str()).collect();
        builder = builder.expand(&expand_refs);
        log::info!("[{}] Query will expand: {:?}", entity_name, expands);
    }
    let query = builder.build();

    let mut result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Query failed for {}: {}", entity_name, e))?;

    loop {
        page += 1;
        let page_start = std::time::Instant::now();

        let page_records = result.data
            .as_ref()
            .map(|d| d.value.len())
            .unwrap_or(0);

        log::info!("[{}] ✅ Page {} fetched in {}ms ({} records)",
            entity_name, page, page_start.elapsed().as_millis(), page_records);

        if let Some(ref data) = result.data {
            all_records.extend(data.value.clone());
        }

        // Report progress
        let progress_msg = match total_count {
            Some(total) => format!("{}/{}", all_records.len(), total),
            None => format!("{} records", all_records.len()),
        };
        if let Some(ref tx) = progress {
            let _ = tx.send(progress_msg);
        }

        // Check if there's more data via nextLink
        if !result.has_more() {
            break;
        }

        // Fetch next page
        let next_start = std::time::Instant::now();
        result = result
            .next_page(&client)
            .await
            .map_err(|e| format!("Pagination failed: {}", e))?
            .ok_or_else(|| "nextLink returned no data".to_string())?;

        log::debug!("[{}] Next page request took {}ms", entity_name, next_start.elapsed().as_millis());
    }

    let total_time = fetch_start.elapsed();
    log::info!(
        "✅ Fetched {} records for {} from {} in {}ms",
        all_records.len(),
        entity_name,
        env_name,
        total_time.as_millis()
    );

    Ok((entity_name, is_source, all_records))
}
