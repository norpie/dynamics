mod error;
mod execution;
/// Step-by-step questionnaire copy implementation
///
/// This module contains the 11-step copy process for duplicating questionnaires
/// across Dynamics 365 environments. Each step creates a specific entity type
/// and tracks created IDs for rollback support.
///
/// ## Module Structure
///
/// - `helpers` - Shared utility functions for data transformation
/// - `error` - Error construction and handling
/// - `execution` - Generic execution logic with automatic batching
/// - `rollback` - Rollback operations for cleanup
/// - `steps` - Individual step implementations (step1-step11)
mod helpers;
mod rollback;
mod steps;

pub mod entity_sets;
pub mod field_specs;

// Re-export public API
pub use steps::{
    step1_create_questionnaire, step2_create_pages, step3_create_page_lines, step4_create_groups,
    step5_create_group_lines, step6_create_questions, step7_create_template_lines,
    step8_create_conditions, step9_create_condition_actions, step10_create_classifications,
    step11_publish_conditions,
};

pub use rollback::rollback_created_entities;

// Re-export helper for use in app.rs
pub use helpers::entity_set_to_friendly_name;
