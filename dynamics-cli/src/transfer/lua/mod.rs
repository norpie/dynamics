//! Lua transform mode for data transfers
//!
//! This module provides a Lua-based transform system as an alternative to
//! declarative field mappings. Users can write Lua scripts that:
//!
//! 1. Declare what data to fetch from source and target environments
//! 2. Transform the data into create/update/delete/deactivate operations
//!
//! # Example Script
//!
//! ```lua
//! local M = {}
//!
//! -- Declare what data we need
//! function M.declare()
//!     return {
//!         source = {
//!             account = {
//!                 fields = { "accountid", "name", "revenue" },
//!                 filter = "statecode eq 0"
//!             }
//!         },
//!         target = {
//!             account = { fields = { "accountid", "name" } }
//!         }
//!     }
//! end
//!
//! -- Transform the data into operations
//! function M.transform(source, target)
//!     local ops = {}
//!     
//!     for _, account in ipairs(source.account) do
//!         local existing = lib.find(target.account, "name", account.name)
//!         
//!         if existing then
//!             table.insert(ops, {
//!                 entity = "account",
//!                 operation = "update",
//!                 id = existing.accountid,
//!                 fields = { revenue = account.revenue }
//!             })
//!         else
//!             table.insert(ops, {
//!                 entity = "account",
//!                 operation = "create",
//!                 fields = { name = account.name, revenue = account.revenue }
//!             })
//!         end
//!     end
//!     
//!     return ops
//! end
//!
//! return M
//! ```
//!
//! # Standard Library
//!
//! Scripts have access to a `lib` namespace with helper functions:
//!
//! - `lib.find(records, field, value)` - Find first matching record
//! - `lib.filter(records, fn)` - Filter records by predicate
//! - `lib.map(records, fn)` - Transform records
//! - `lib.group_by(records, field)` - Group records by field value
//! - `lib.guid()` - Generate new GUID
//! - `lib.is_guid(value)` - Check if valid GUID
//! - `lib.lower(s)`, `lib.upper(s)`, `lib.trim(s)` - String functions
//! - `lib.split(s, delim)` - Split string
//! - `lib.contains(s, sub)` - Substring check
//! - `lib.now()` - Current ISO datetime
//! - `lib.is_nil(v)`, `lib.is_string(v)`, etc. - Type checks
//! - `lib.log(msg)`, `lib.warn(msg)` - Logging
//! - `lib.status(msg)`, `lib.progress(current, total)` - Progress updates

mod types;
mod runtime;
mod stdlib;
mod validate;
mod execute;

// Re-export public types
pub use types::{Declaration, EntityDeclaration, LuaOperation, OperationType};
pub use runtime::LuaRuntime;
pub use stdlib::{LogMessage, StatusUpdate, StdlibContext};
pub use validate::{ValidationError, ValidationResult, validate_script, validate_script_execution};
pub use execute::{
    ExecutionContext, ExecutionResult, ExecutionUpdate,
    execute_transform, execute_transform_async, run_declare, validate_operations,
};
