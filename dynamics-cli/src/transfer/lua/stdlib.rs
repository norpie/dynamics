//! Standard library functions for Lua scripts
//!
//! Implements the `lib.*` namespace available in transform scripts.

use mlua::{Function, Lua, Result as LuaResult, Table, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Messages captured from lib.log/lib.warn calls
#[derive(Debug, Clone)]
pub enum LogMessage {
    Info(String),
    Warn(String),
}

/// Status updates from lib.status/lib.progress calls
#[derive(Debug, Clone)]
pub enum StatusUpdate {
    Status(String),
    Progress { current: usize, total: usize },
}

/// Context for stdlib functions that need to communicate with the host
#[derive(Debug, Default)]
pub struct StdlibContext {
    /// Captured log messages
    pub logs: Vec<LogMessage>,
    /// Latest status update
    pub status: Option<StatusUpdate>,
}

/// Register the `lib` table with all standard library functions
pub fn register_stdlib(lua: &Lua, context: Arc<Mutex<StdlibContext>>) -> LuaResult<()> {
    let lib = lua.create_table()?;

    // Collection functions
    lib.set("find", create_find_fn(lua)?)?;
    lib.set("filter", create_filter_fn(lua)?)?;
    lib.set("map", create_map_fn(lua)?)?;
    lib.set("group_by", create_group_by_fn(lua)?)?;

    // GUID functions
    lib.set("guid", create_guid_fn(lua)?)?;
    lib.set("is_guid", create_is_guid_fn(lua)?)?;

    // String functions
    lib.set("lower", create_lower_fn(lua)?)?;
    lib.set("upper", create_upper_fn(lua)?)?;
    lib.set("trim", create_trim_fn(lua)?)?;
    lib.set("split", create_split_fn(lua)?)?;
    lib.set("contains", create_contains_fn(lua)?)?;
    lib.set("starts_with", create_starts_with_fn(lua)?)?;
    lib.set("ends_with", create_ends_with_fn(lua)?)?;

    // Date functions
    lib.set("now", create_now_fn(lua)?)?;
    lib.set("parse_date", create_parse_date_fn(lua)?)?;
    lib.set("format_date", create_format_date_fn(lua)?)?;

    // Type check functions
    lib.set("is_nil", create_is_nil_fn(lua)?)?;
    lib.set("is_string", create_is_string_fn(lua)?)?;
    lib.set("is_number", create_is_number_fn(lua)?)?;
    lib.set("is_table", create_is_table_fn(lua)?)?;
    lib.set("is_boolean", create_is_boolean_fn(lua)?)?;

    // Logging functions (with context)
    let ctx = context.clone();
    lib.set("log", create_log_fn(lua, ctx)?)?;
    let ctx = context.clone();
    lib.set("warn", create_warn_fn(lua, ctx)?)?;

    // Status functions (with context)
    let ctx = context.clone();
    lib.set("status", create_status_fn(lua, ctx)?)?;
    let ctx = context;
    lib.set("progress", create_progress_fn(lua, ctx)?)?;

    lua.globals().set("lib", lib)?;
    Ok(())
}

// =============================================================================
// Collection functions
// =============================================================================

/// lib.find(records, field, value) -> record|nil
/// Find first record where record[field] == value
fn create_find_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (records, field, value): (Table, String, Value)| {
        for pair in records.pairs::<Value, Table>() {
            if let Ok((_, record)) = pair {
                if let Ok(field_value) = record.get::<Value>(field.as_str()) {
                    if values_equal(&field_value, &value) {
                        return Ok(Value::Table(record));
                    }
                }
            }
        }
        Ok(Value::Nil)
    })
}

/// lib.filter(records, fn) -> records
/// Filter records by predicate function
fn create_filter_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (records, predicate): (Table, Function)| {
        let result = lua.create_table()?;
        let mut idx = 1;
        for pair in records.pairs::<Value, Value>() {
            if let Ok((_, record)) = pair {
                let keep: bool = predicate.call(record.clone())?;
                if keep {
                    result.set(idx, record)?;
                    idx += 1;
                }
            }
        }
        Ok(result)
    })
}

/// lib.map(records, fn) -> records
/// Transform each record using function
fn create_map_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (records, transform): (Table, Function)| {
        let result = lua.create_table()?;
        let mut idx = 1;
        for pair in records.pairs::<Value, Value>() {
            if let Ok((_, record)) = pair {
                let transformed: Value = transform.call(record)?;
                result.set(idx, transformed)?;
                idx += 1;
            }
        }
        Ok(result)
    })
}

/// lib.group_by(records, field) -> table
/// Group records by field value
fn create_group_by_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (records, field): (Table, String)| {
        let result = lua.create_table()?;
        
        for pair in records.pairs::<Value, Table>() {
            if let Ok((_, record)) = pair {
                if let Ok(key) = record.get::<Value>(field.as_str()) {
                    let key_str = value_to_string(&key);
                    
                    // Get or create the group
                    let group: Table = match result.get::<Table>(key_str.as_str()) {
                        Ok(g) => g,
                        Err(_) => {
                            let g = lua.create_table()?;
                            result.set(key_str.as_str(), g.clone())?;
                            g
                        }
                    };
                    
                    // Add record to group
                    let len = group.len()? + 1;
                    group.set(len, record)?;
                }
            }
        }
        Ok(result)
    })
}

// =============================================================================
// GUID functions
// =============================================================================

/// lib.guid() -> string
/// Generate a new random GUID
fn create_guid_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, ()| {
        Ok(Uuid::new_v4().to_string())
    })
}

/// lib.is_guid(value) -> bool
/// Check if value is a valid GUID string
fn create_is_guid_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, value: Value| {
        match value {
            Value::String(s) => {
                let str_ref = s.to_str()?;
                Ok(Uuid::parse_str(str_ref.as_ref()).is_ok())
            }
            _ => Ok(false),
        }
    })
}

// =============================================================================
// String functions
// =============================================================================

/// lib.lower(s) -> string
fn create_lower_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(s.to_lowercase()))
}

/// lib.upper(s) -> string
fn create_upper_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(s.to_uppercase()))
}

/// lib.trim(s) -> string
fn create_trim_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, s: String| Ok(s.trim().to_string()))
}

/// lib.split(s, delim) -> table
fn create_split_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, (s, delim): (String, String)| {
        let result = lua.create_table()?;
        for (i, part) in s.split(&delim).enumerate() {
            result.set(i + 1, part)?;
        }
        Ok(result)
    })
}

/// lib.contains(s, sub) -> bool
fn create_contains_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (s, sub): (String, String)| Ok(s.contains(&sub)))
}

/// lib.starts_with(s, prefix) -> bool
fn create_starts_with_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (s, prefix): (String, String)| Ok(s.starts_with(&prefix)))
}

/// lib.ends_with(s, suffix) -> bool
fn create_ends_with_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (s, suffix): (String, String)| Ok(s.ends_with(&suffix)))
}

// =============================================================================
// Date functions
// =============================================================================

/// lib.now() -> string
/// Returns current UTC time in ISO 8601 format
fn create_now_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, ()| {
        Ok(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
    })
}

/// lib.parse_date(s) -> string|nil
/// Parse various date formats to ISO 8601
fn create_parse_date_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, s: String| {
        // Try common formats
        let formats = [
            "%Y-%m-%dT%H:%M:%S%.fZ",
            "%Y-%m-%dT%H:%M:%SZ",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d",
            "%d/%m/%Y %H:%M:%S",
            "%d/%m/%Y",
            "%m/%d/%Y %H:%M:%S",
            "%m/%d/%Y",
        ];
        
        for fmt in formats {
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s, fmt) {
                let result = dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                return Ok(Value::String(lua.create_string(&result)?));
            }
            // Try date only
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, fmt) {
                let result = d.format("%Y-%m-%dT00:00:00Z").to_string();
                return Ok(Value::String(lua.create_string(&result)?));
            }
        }
        
        Ok(Value::Nil)
    })
}

/// lib.format_date(dt, fmt) -> string|nil
/// Format ISO date string with given format
fn create_format_date_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, (dt, fmt): (String, String)| {
        if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%SZ") {
            Ok(Some(parsed.format(&fmt).to_string()))
        } else if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%S%.fZ") {
            Ok(Some(parsed.format(&fmt).to_string()))
        } else {
            Ok(None)
        }
    })
}

// =============================================================================
// Type check functions
// =============================================================================

/// lib.is_nil(v) -> bool
fn create_is_nil_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Nil)))
}

/// lib.is_string(v) -> bool
fn create_is_string_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::String(_))))
}

/// lib.is_number(v) -> bool
fn create_is_number_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Number(_) | Value::Integer(_))))
}

/// lib.is_table(v) -> bool
fn create_is_table_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Table(_))))
}

/// lib.is_boolean(v) -> bool
fn create_is_boolean_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|_, v: Value| Ok(matches!(v, Value::Boolean(_))))
}

// =============================================================================
// Logging functions
// =============================================================================

/// lib.log(msg) - Info log
fn create_log_fn(lua: &Lua, context: Arc<Mutex<StdlibContext>>) -> LuaResult<Function> {
    lua.create_function(move |_, msg: String| {
        if let Ok(mut ctx) = context.lock() {
            ctx.logs.push(LogMessage::Info(msg));
        }
        Ok(())
    })
}

/// lib.warn(msg) - Warning log
fn create_warn_fn(lua: &Lua, context: Arc<Mutex<StdlibContext>>) -> LuaResult<Function> {
    lua.create_function(move |_, msg: String| {
        if let Ok(mut ctx) = context.lock() {
            ctx.logs.push(LogMessage::Warn(msg));
        }
        Ok(())
    })
}

// =============================================================================
// Status functions
// =============================================================================

/// lib.status(msg) - Update status display
fn create_status_fn(lua: &Lua, context: Arc<Mutex<StdlibContext>>) -> LuaResult<Function> {
    lua.create_function(move |_, msg: String| {
        if let Ok(mut ctx) = context.lock() {
            ctx.status = Some(StatusUpdate::Status(msg));
        }
        Ok(())
    })
}

/// lib.progress(current, total) - Update progress bar
fn create_progress_fn(lua: &Lua, context: Arc<Mutex<StdlibContext>>) -> LuaResult<Function> {
    lua.create_function(move |_, (current, total): (usize, usize)| {
        if let Ok(mut ctx) = context.lock() {
            ctx.status = Some(StatusUpdate::Progress { current, total });
        }
        Ok(())
    })
}

// =============================================================================
// Helper functions
// =============================================================================

/// Compare two Lua values for equality
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
        (Value::Integer(a), Value::Number(b)) | (Value::Number(b), Value::Integer(a)) => {
            (*a as f64 - b).abs() < f64::EPSILON
        }
        (Value::String(a), Value::String(b)) => a.as_bytes() == b.as_bytes(),
        _ => false,
    }
}

/// Convert a Lua value to a string key
fn value_to_string(v: &Value) -> String {
    match v {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_str().map(|bs| bs.to_string()).unwrap_or_default(),
        _ => format!("{:?}", v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_lua() -> (Lua, Arc<Mutex<StdlibContext>>) {
        let lua = Lua::new();
        let context = Arc::new(Mutex::new(StdlibContext::default()));
        register_stdlib(&lua, context.clone()).unwrap();
        (lua, context)
    }

    #[test]
    fn test_guid_functions() {
        let (lua, _) = create_test_lua();
        
        // Test guid generation
        let guid: String = lua.load("return lib.guid()").eval().unwrap();
        assert!(Uuid::parse_str(&guid).is_ok());
        
        // Test is_guid
        let is_valid: bool = lua.load("return lib.is_guid('550e8400-e29b-41d4-a716-446655440000')").eval().unwrap();
        assert!(is_valid);
        
        let is_invalid: bool = lua.load("return lib.is_guid('not-a-guid')").eval().unwrap();
        assert!(!is_invalid);
    }

    #[test]
    fn test_string_functions() {
        let (lua, _) = create_test_lua();
        
        let lower: String = lua.load("return lib.lower('HELLO')").eval().unwrap();
        assert_eq!(lower, "hello");
        
        let upper: String = lua.load("return lib.upper('hello')").eval().unwrap();
        assert_eq!(upper, "HELLO");
        
        let trimmed: String = lua.load("return lib.trim('  hello  ')").eval().unwrap();
        assert_eq!(trimmed, "hello");
        
        let contains: bool = lua.load("return lib.contains('hello world', 'world')").eval().unwrap();
        assert!(contains);
        
        let starts: bool = lua.load("return lib.starts_with('hello world', 'hello')").eval().unwrap();
        assert!(starts);
        
        let ends: bool = lua.load("return lib.ends_with('hello world', 'world')").eval().unwrap();
        assert!(ends);
    }

    #[test]
    fn test_split() {
        let (lua, _) = create_test_lua();
        
        let result: Vec<String> = lua.load(r#"
            local parts = lib.split('a,b,c', ',')
            return { parts[1], parts[2], parts[3] }
        "#).eval().unwrap();
        
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_type_checks() {
        let (lua, _) = create_test_lua();
        
        let is_nil: bool = lua.load("return lib.is_nil(nil)").eval().unwrap();
        assert!(is_nil);
        
        let is_string: bool = lua.load("return lib.is_string('hello')").eval().unwrap();
        assert!(is_string);
        
        let is_number: bool = lua.load("return lib.is_number(42)").eval().unwrap();
        assert!(is_number);
        
        let is_table: bool = lua.load("return lib.is_table({})").eval().unwrap();
        assert!(is_table);
        
        let is_bool: bool = lua.load("return lib.is_boolean(true)").eval().unwrap();
        assert!(is_bool);
    }

    #[test]
    fn test_find() {
        let (lua, _) = create_test_lua();
        
        let result: String = lua.load(r#"
            local records = {
                { name = "Alice", age = 30 },
                { name = "Bob", age = 25 },
                { name = "Charlie", age = 35 }
            }
            local found = lib.find(records, "name", "Bob")
            return found.age
        "#).eval().unwrap();
        
        assert_eq!(result, "25");
    }

    #[test]
    fn test_filter() {
        let (lua, _) = create_test_lua();
        
        let count: i32 = lua.load(r#"
            local records = {
                { name = "Alice", age = 30 },
                { name = "Bob", age = 25 },
                { name = "Charlie", age = 35 }
            }
            local filtered = lib.filter(records, function(r) return r.age >= 30 end)
            return #filtered
        "#).eval().unwrap();
        
        assert_eq!(count, 2);
    }

    #[test]
    fn test_map() {
        let (lua, _) = create_test_lua();
        
        let result: Vec<String> = lua.load(r#"
            local records = {
                { name = "Alice" },
                { name = "Bob" }
            }
            local names = lib.map(records, function(r) return r.name end)
            return { names[1], names[2] }
        "#).eval().unwrap();
        
        assert_eq!(result, vec!["Alice", "Bob"]);
    }

    #[test]
    fn test_group_by() {
        let (lua, _) = create_test_lua();
        
        let count_a: i32 = lua.load(r#"
            local records = {
                { name = "Alice", dept = "A" },
                { name = "Bob", dept = "B" },
                { name = "Charlie", dept = "A" }
            }
            local groups = lib.group_by(records, "dept")
            return #groups["A"]
        "#).eval().unwrap();
        
        assert_eq!(count_a, 2);
    }

    #[test]
    fn test_logging() {
        let (lua, context) = create_test_lua();
        
        lua.load(r#"
            lib.log("Info message")
            lib.warn("Warning message")
        "#).exec().unwrap();
        
        let ctx = context.lock().unwrap();
        assert_eq!(ctx.logs.len(), 2);
        assert!(matches!(&ctx.logs[0], LogMessage::Info(s) if s == "Info message"));
        assert!(matches!(&ctx.logs[1], LogMessage::Warn(s) if s == "Warning message"));
    }

    #[test]
    fn test_status() {
        let (lua, context) = create_test_lua();
        
        lua.load(r#"lib.status("Processing...")"#).exec().unwrap();
        
        let ctx = context.lock().unwrap();
        assert!(matches!(&ctx.status, Some(StatusUpdate::Status(s)) if s == "Processing..."));
    }

    #[test]
    fn test_progress() {
        let (lua, context) = create_test_lua();
        
        lua.load(r#"lib.progress(50, 100)"#).exec().unwrap();
        
        let ctx = context.lock().unwrap();
        assert!(matches!(&ctx.status, Some(StatusUpdate::Progress { current: 50, total: 100 })));
    }

    #[test]
    fn test_now() {
        let (lua, _) = create_test_lua();
        
        let now: String = lua.load("return lib.now()").eval().unwrap();
        // Should be in ISO format
        assert!(now.contains("T"));
        assert!(now.ends_with("Z"));
    }
}
