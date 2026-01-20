//! Step views for the Entity Sync App
//!
//! Each step is a pure view function that takes state and theme,
//! returning an Element<Msg>.

pub mod analysis;
pub mod confirm;
pub mod diff_review;
pub mod entity_select;
pub mod environment_select;

pub use analysis::*;
pub use confirm::*;
pub use diff_review::*;
pub use entity_select::*;
pub use environment_select::*;
