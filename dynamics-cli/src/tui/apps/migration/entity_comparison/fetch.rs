//! Entity metadata fetching with caching

use super::FetchedData;

/// Type of data to fetch
pub enum FetchType {
    SourceFields,
    SourceForms,
    SourceViews,
    TargetFields,
    TargetForms,
    TargetViews,
}

/// Fetch specific metadata type with optional 12-hour caching
pub async fn fetch_with_cache(
    environment_name: &str,
    entity_name: &str,
    fetch_type: FetchType,
    use_cache: bool,
) -> Result<FetchedData, String> {
    let config = crate::global_config();
    let manager = crate::client_manager();

    // Check cache first (12 hours) - use full metadata cache, only if use_cache is true
    if use_cache {
        let cached_metadata = config.get_entity_metadata_cache(environment_name, entity_name, 12).await
            .ok()
            .flatten();

        // If we have cached metadata, extract the requested type
        // Note: Cached metadata already has relationships extracted and lookup fields removed
        if let Some(cached) = cached_metadata {
            return match fetch_type {
                FetchType::SourceFields | FetchType::TargetFields => {
                    // Apply virtual field filtering to cached fields too
                    // (in case cache was created before this filtering was added)
                    let relationship_names: std::collections::HashSet<String> = cached.relationships.iter()
                        .map(|r| r.name.clone())
                        .collect();
                    let fields = process_lookup_fields(cached.fields, &relationship_names);

                    match fetch_type {
                        FetchType::SourceFields => Ok(FetchedData::SourceFields(entity_name.to_string(), fields)),
                        FetchType::TargetFields => Ok(FetchedData::TargetFields(entity_name.to_string(), fields)),
                        _ => unreachable!()
                    }
                }
                FetchType::SourceForms => Ok(FetchedData::SourceForms(entity_name.to_string(), cached.forms)),
                FetchType::SourceViews => Ok(FetchedData::SourceViews(entity_name.to_string(), cached.views)),
                FetchType::TargetForms => Ok(FetchedData::TargetForms(entity_name.to_string(), cached.forms)),
                FetchType::TargetViews => Ok(FetchedData::TargetViews(entity_name.to_string(), cached.views)),
            };
        }
    }

    // Fetch from API
    let client = manager.get_client(environment_name).await
        .map_err(|e| e.to_string())?;

    match fetch_type {
        FetchType::SourceFields => {
            let fields = client.fetch_entity_fields_combined(entity_name).await.map_err(|e| e.to_string())?;
            // Extract relationships to get their names for filtering virtual fields
            let relationships = extract_relationships(&fields);
            let relationship_names: std::collections::HashSet<String> = relationships.iter()
                .map(|r| r.name.clone())
                .collect();
            // Filter virtual fields using relationship names
            let fields = process_lookup_fields(fields, &relationship_names);
            Ok(FetchedData::SourceFields(entity_name.to_string(), fields))
        }
        FetchType::SourceForms => {
            let forms = client.fetch_entity_forms(entity_name).await.map_err(|e| e.to_string())?;
            Ok(FetchedData::SourceForms(entity_name.to_string(), forms))
        }
        FetchType::SourceViews => {
            let views = client.fetch_entity_views(entity_name).await.map_err(|e| e.to_string())?;
            Ok(FetchedData::SourceViews(entity_name.to_string(), views))
        }
        FetchType::TargetFields => {
            let fields = client.fetch_entity_fields_combined(entity_name).await.map_err(|e| e.to_string())?;
            // Extract relationships to get their names for filtering virtual fields
            let relationships = extract_relationships(&fields);
            let relationship_names: std::collections::HashSet<String> = relationships.iter()
                .map(|r| r.name.clone())
                .collect();
            // Filter virtual fields using relationship names
            let fields = process_lookup_fields(fields, &relationship_names);
            Ok(FetchedData::TargetFields(entity_name.to_string(), fields))
        }
        FetchType::TargetForms => {
            let forms = client.fetch_entity_forms(entity_name).await.map_err(|e| e.to_string())?;
            Ok(FetchedData::TargetForms(entity_name.to_string(), forms))
        }
        FetchType::TargetViews => {
            let views = client.fetch_entity_views(entity_name).await.map_err(|e| e.to_string())?;
            Ok(FetchedData::TargetViews(entity_name.to_string(), views))
        }
    }
}

/// Extract unique entity types from relationships with usage counts
/// Returns list of (entity_name, usage_count) tuples, sorted by name
pub fn extract_entities(relationships: &[crate::api::metadata::RelationshipMetadata]) -> Vec<(String, usize)> {
    use std::collections::HashMap;

    let mut entity_counts: HashMap<String, usize> = HashMap::new();

    for rel in relationships {
        // Skip unknown/empty entity names
        if rel.related_entity.is_empty() || rel.related_entity == "unknown" {
            continue;
        }

        *entity_counts.entry(rel.related_entity.clone()).or_insert(0) += 1;
    }

    let mut entities: Vec<(String, usize)> = entity_counts.into_iter().collect();
    entities.sort_by(|a, b| a.0.cmp(&b.0));

    entities
}

/// Extract relationships from field list
/// Includes all Lookup fields and NavigationProperties (collection relationships)
pub fn extract_relationships(fields: &[crate::api::metadata::FieldMetadata]) -> Vec<crate::api::metadata::RelationshipMetadata> {
    fields.iter()
        .filter_map(|f| {
            // Check for Lookup (ManyToOne) or Relationship:* (OneToMany/ManyToMany)
            let relationship_type = match &f.field_type {
                crate::api::metadata::FieldType::Lookup => {
                    Some(crate::api::metadata::RelationshipType::ManyToOne)
                }
                crate::api::metadata::FieldType::Other(t) if t.starts_with("Relationship:") => {
                    // Extract relationship type from "Relationship:OneToMany"
                    if t.contains("OneToMany") {
                        Some(crate::api::metadata::RelationshipType::OneToMany)
                    } else if t.contains("ManyToMany") {
                        Some(crate::api::metadata::RelationshipType::ManyToMany)
                    } else {
                        Some(crate::api::metadata::RelationshipType::ManyToOne)
                    }
                }
                _ => None,
            };

            relationship_type.map(|rel_type| crate::api::metadata::RelationshipMetadata {
                name: f.logical_name.clone(),
                relationship_type: rel_type,
                related_entity: f.related_entity.clone().unwrap_or_else(|| "unknown".to_string()),
                related_attribute: f.logical_name.clone(),
            })
        })
        .collect()
}

/// Process lookup fields by filtering out _*_value virtual fields and ensuring
/// base lookup fields are properly typed
///
/// Also filters out relationship-derived virtual fields (e.g., customeridname, customeridyominame)
/// by using the provided relationship names.
fn process_lookup_fields(
    fields: Vec<crate::api::metadata::FieldMetadata>,
    relationship_names: &std::collections::HashSet<String>
) -> Vec<crate::api::metadata::FieldMetadata> {
    use std::collections::HashMap;

    log::info!("🔍 Processing {} fields with {} known relationships", fields.len(), relationship_names.len());
    if !relationship_names.is_empty() {
        let mut rel_vec: Vec<_> = relationship_names.iter().collect();
        rel_vec.sort();
        log::info!("📋 Relationships to check for virtual fields: {:?}", rel_vec);
    }

    // Build map of _*_value fields to their related entity info
    let mut value_field_map: HashMap<String, Option<String>> = HashMap::new();

    for field in &fields {
        // Detect _*_value pattern
        if field.logical_name.starts_with('_') && field.logical_name.ends_with("_value") {
            // Extract base field name: _cgk_deadlineid_value -> cgk_deadlineid
            if let Some(base_name) = field.logical_name
                .strip_prefix('_')
                .and_then(|s| s.strip_suffix("_value"))
            {
                value_field_map.insert(base_name.to_string(), field.related_entity.clone());
            }
        }
    }

    // Filter and update fields
    let original_count = fields.len();
    let filtered_fields: Vec<_> = fields.into_iter()
        .filter_map(|mut field| {
            // Filter out _*_value fields
            if field.logical_name.starts_with('_') && field.logical_name.ends_with("_value") {
                log::debug!("Filtering out virtual lookup value field: {}", field.logical_name);
                return None;
            }

            // Filter out Virtual type fields (formatted display values)
            // These include *name, *yominame suffixes for lookups and optionsets
            if matches!(field.field_type, crate::api::metadata::FieldType::Other(ref t) if t == "Virtual") {
                log::debug!("Filtering out virtual display field: {}", field.logical_name);
                return None;
            }

            // Filter out relationship-derived virtual fields (e.g., customeridname, customeridyominame)
            // These are fields that start with a relationship name but have a suffix
            for rel_name in relationship_names {
                if field.logical_name != *rel_name && field.logical_name.starts_with(rel_name) {
                    log::info!("✂️ Filtering relationship-derived virtual field: '{}' (derived from relationship '{}')",
                               field.logical_name, rel_name);
                    return None;
                }
            }

            // Check if this field has a corresponding _*_value field
            if let Some(related_entity) = value_field_map.get(&field.logical_name) {
                // Ensure field is marked as Lookup
                if !matches!(field.field_type, crate::api::metadata::FieldType::Lookup) {
                    log::debug!("Converting field {} to Lookup type (found _*_value)", field.logical_name);
                    field.field_type = crate::api::metadata::FieldType::Lookup;
                }

                // Update related_entity if we extracted it from the _value field
                if field.related_entity.is_none() && related_entity.is_some() {
                    field.related_entity = related_entity.clone();
                }
            }

            Some(field)
        })
        .collect();

    let filtered_count = original_count - filtered_fields.len();
    if filtered_count > 0 {
        log::info!("✅ Filtered {} virtual fields, {} fields remaining", filtered_count, filtered_fields.len());
    }

    filtered_fields
}

/// Fetch example record data for a pair
pub async fn fetch_example_pair_data(
    source_env: &str,
    source_entity: &str,
    source_record_id: &str,
    target_env: &str,
    target_entity: &str,
    target_record_id: &str,
) -> Result<(serde_json::Value, serde_json::Value), String> {
    log::debug!("Fetching example pair: source={}:{} ({}), target={}:{} ({})",
        source_env, source_entity, source_record_id,
        target_env, target_entity, target_record_id);

    let manager = crate::client_manager();

    // Fetch source record
    let source_client = manager.get_client(source_env)
        .await
        .map_err(|e| format!("Failed to get source client: {}", e))?;

    log::debug!("Fetching source record...");
    let source_record = source_client.fetch_record_by_id(source_entity, source_record_id)
        .await
        .map_err(|e| format!("Failed to fetch source record: {}", e))?;
    log::debug!("Source record fetched successfully");

    // Fetch target record
    let target_client = manager.get_client(target_env)
        .await
        .map_err(|e| format!("Failed to get target client: {}", e))?;

    log::debug!("Fetching target record...");
    let target_record = target_client.fetch_record_by_id(target_entity, target_record_id)
        .await
        .map_err(|e| format!("Failed to fetch target record: {}", e))?;
    log::debug!("Target record fetched successfully");

    Ok((source_record, target_record))
}
