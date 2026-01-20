/// Error handling and error construction
use super::super::models::{CopyError, CopyPhase};
use super::helpers::entity_set_to_friendly_name;
use std::collections::HashMap;

/// Build a CopyError from error details and partial progress
pub fn build_error(
    message: String,
    phase: CopyPhase,
    step: usize,
    created_ids: &[(String, String)],
) -> CopyError {
    // Calculate how many of each entity type were created before the error
    let mut partial_counts = HashMap::new();
    for (entity_set, _) in created_ids {
        let friendly_name = entity_set_to_friendly_name(entity_set.as_str());
        *partial_counts.entry(friendly_name.to_string()).or_insert(0) += 1;
    }

    CopyError {
        error_message: message,
        phase,
        step,
        partial_counts,
        rollback_complete: false,
        orphaned_entities_csv: None,
    }
}
