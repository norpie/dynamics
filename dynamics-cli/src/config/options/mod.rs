//! Options system for persistent, type-safe configuration
//!
//! The options system provides:
//! - Type-safe storage with validation
//! - Namespaced organization
//! - Self-describing metadata for UI generation
//! - Database-backed persistence

pub mod builder;
pub mod registrations;
pub mod registry;
pub mod store;
pub mod types;

pub use builder::OptionDefBuilder;
pub use registry::OptionsRegistry;
pub use store::Options;
pub use types::{OptionDefinition, OptionType, OptionValue};
