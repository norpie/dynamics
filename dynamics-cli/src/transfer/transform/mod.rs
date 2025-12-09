//! Transform engine for applying field mappings to source records

mod path;
mod apply;
mod engine;

pub use path::resolve_path;
pub use apply::apply_transform;
pub use engine::{TransformEngine, TransformContext, TransformError};
