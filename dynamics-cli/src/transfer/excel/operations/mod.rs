//! Excel import for operations (Create/Update/Delete)
//!
//! Sheet naming convention:
//! - "Create (entity)" for POST operations
//! - "Update (entity)" for PATCH operations (requires primary key column)
//! - "Delete (entity)" for DELETE operations (requires primary key column)
//!
//! Primary key detection:
//! - For Update/Delete, looks for column named "{entity_singular}id" (e.g., "nrq_capacityid" for "nrq_capacities")

mod reader;

pub use reader::{read_operations_excel, OperationType, ParsedOperations, SheetOperations};
