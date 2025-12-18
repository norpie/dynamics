//! Lua runtime for transform scripts
//!
//! Provides a sandboxed Lua environment for running transform scripts.

use anyhow::{Context, Result};
use mlua::{Function, Lua, StdLib, Table, Value};
use std::sync::{Arc, Mutex};

use super::stdlib::{register_stdlib, StdlibContext};
use super::types::Declaration;

/// A sandboxed Lua runtime for executing transform scripts
pub struct LuaRuntime {
    lua: Lua,
    context: Arc<Mutex<StdlibContext>>,
}

impl LuaRuntime {
    /// Create a new sandboxed Lua runtime
    pub fn new() -> Result<Self> {
        // Create Lua with limited standard libraries (no io, os, debug, etc.)
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            mlua::LuaOptions::default(),
        )
        .context("Failed to create Lua runtime")?;

        // Set memory limit (1GB - transforms can handle large datasets)
        lua.set_memory_limit(1024 * 1024 * 1024)?;

        let context = Arc::new(Mutex::new(StdlibContext::default()));

        // Register our standard library
        register_stdlib(&lua, context.clone()).context("Failed to register stdlib")?;

        Ok(LuaRuntime { lua, context })
    }

    /// Load and validate a script
    /// Returns the module table if successful
    pub fn load_script(&self, script: &str) -> Result<Table> {
        // Load the script as a chunk that returns a module table
        let chunk = self.lua.load(script);
        let module: Table = chunk
            .eval()
            .context("Script must return a module table (e.g., 'return M')")?;

        // Verify required functions exist
        self.get_declare_fn(&module)?;
        self.get_transform_fn(&module)?;

        Ok(module)
    }

    /// Get the M.declare function from a loaded module
    pub fn get_declare_fn<'a>(&'a self, module: &'a Table) -> Result<Function> {
        module
            .get::<Function>("declare")
            .context("Script must have a 'declare' function (M.declare)")
    }

    /// Get the M.transform function from a loaded module
    pub fn get_transform_fn<'a>(&'a self, module: &'a Table) -> Result<Function> {
        module
            .get::<Function>("transform")
            .context("Script must have a 'transform' function (M.transform)")
    }

    /// Run the declare function and parse the result
    pub fn run_declare(&self, module: &Table) -> Result<Declaration> {
        let declare_fn = self.get_declare_fn(module)?;
        let result: Table = declare_fn
            .call(())
            .context("Failed to call declare()")?;

        self.parse_declaration(result)
    }

    /// Parse a Lua table into a Declaration struct
    fn parse_declaration(&self, table: Table) -> Result<Declaration> {
        let mut declaration = Declaration::default();

        // Parse source entities
        if let Ok(source) = table.get::<Table>("source") {
            for pair in source.pairs::<String, Table>() {
                let (entity_name, entity_table) = pair.context("Invalid source entity")?;
                declaration
                    .source
                    .insert(entity_name, self.parse_entity_declaration(entity_table)?);
            }
        }

        // Parse target entities
        if let Ok(target) = table.get::<Table>("target") {
            for pair in target.pairs::<String, Table>() {
                let (entity_name, entity_table) = pair.context("Invalid target entity")?;
                declaration
                    .target
                    .insert(entity_name, self.parse_entity_declaration(entity_table)?);
            }
        }

        Ok(declaration)
    }

    /// Parse a single entity declaration
    fn parse_entity_declaration(
        &self,
        table: Table,
    ) -> Result<super::types::EntityDeclaration> {
        let mut decl = super::types::EntityDeclaration::default();

        // Parse fields array
        if let Ok(fields) = table.get::<Table>("fields") {
            for pair in fields.pairs::<i64, String>() {
                if let Ok((_, field)) = pair {
                    decl.fields.push(field);
                }
            }
        }

        // Parse expand array
        if let Ok(expand) = table.get::<Table>("expand") {
            for pair in expand.pairs::<i64, String>() {
                if let Ok((_, field)) = pair {
                    decl.expand.push(field);
                }
            }
        }

        // Parse filter
        if let Ok(filter) = table.get::<String>("filter") {
            decl.filter = Some(filter);
        }

        // Parse top
        if let Ok(top) = table.get::<i64>("top") {
            decl.top = Some(top as usize);
        }

        Ok(decl)
    }

    /// Run the transform function with source and target data
    pub fn run_transform(
        &self,
        module: &Table,
        source_data: &serde_json::Value,
        target_data: &serde_json::Value,
    ) -> Result<Vec<super::types::LuaOperation>> {
        let transform_fn = self.get_transform_fn(module)?;

        // Convert JSON to Lua tables
        let source_table = self.json_to_lua(source_data)?;
        let target_table = self.json_to_lua(target_data)?;

        // Call transform(source, target)
        let result: Table = transform_fn
            .call((source_table, target_table))
            .context("Failed to call transform(source, target)")?;

        // Parse operations
        self.parse_operations(result)
    }

    /// Convert JSON value to Lua value
    pub fn json_to_lua(&self, value: &serde_json::Value) -> Result<Value> {
        match value {
            serde_json::Value::Null => Ok(Value::Nil),
            serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Value::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Number(f))
                } else {
                    Ok(Value::Nil)
                }
            }
            serde_json::Value::String(s) => {
                Ok(Value::String(self.lua.create_string(s)?))
            }
            serde_json::Value::Array(arr) => {
                let table = self.lua.create_table()?;
                for (i, item) in arr.iter().enumerate() {
                    table.set(i + 1, self.json_to_lua(item)?)?;
                }
                Ok(Value::Table(table))
            }
            serde_json::Value::Object(obj) => {
                let table = self.lua.create_table()?;
                for (key, val) in obj {
                    table.set(key.as_str(), self.json_to_lua(val)?)?;
                }
                Ok(Value::Table(table))
            }
        }
    }

    /// Convert Lua value to JSON
    pub fn lua_to_json(&self, value: Value) -> Result<serde_json::Value> {
        match value {
            Value::Nil => Ok(serde_json::Value::Null),
            Value::Boolean(b) => Ok(serde_json::Value::Bool(b)),
            Value::Integer(i) => Ok(serde_json::json!(i)),
            Value::Number(n) => Ok(serde_json::json!(n)),
            Value::String(s) => Ok(serde_json::Value::String(s.to_str()?.to_string())),
            Value::Table(t) => {
                // Check if it's an array (sequential integer keys starting at 1)
                let len = t.len()?;
                if len > 0 {
                    // Try to treat as array
                    let mut arr = Vec::new();
                    let mut is_array = true;
                    for i in 1..=len {
                        match t.get::<Value>(i) {
                            Ok(v) => arr.push(self.lua_to_json(v)?),
                            Err(_) => {
                                is_array = false;
                                break;
                            }
                        }
                    }
                    if is_array {
                        return Ok(serde_json::Value::Array(arr));
                    }
                }

                // Treat as object
                let mut obj = serde_json::Map::new();
                for pair in t.pairs::<Value, Value>() {
                    let (k, v) = pair?;
                    let key = match k {
                        Value::String(s) => s.to_str()?.to_string(),
                        Value::Integer(i) => i.to_string(),
                        _ => continue,
                    };
                    obj.insert(key, self.lua_to_json(v)?);
                }
                Ok(serde_json::Value::Object(obj))
            }
            _ => Ok(serde_json::Value::Null),
        }
    }

    /// Parse the operations array returned by transform()
    fn parse_operations(&self, table: Table) -> Result<Vec<super::types::LuaOperation>> {
        let mut operations = Vec::new();

        for pair in table.pairs::<i64, Table>() {
            let (_, op_table) = pair.context("Invalid operation in result")?;
            let op = self.parse_single_operation(op_table)?;
            operations.push(op);
        }

        Ok(operations)
    }

    /// Parse a single operation table
    fn parse_single_operation(&self, table: Table) -> Result<super::types::LuaOperation> {
        use super::types::{LuaOperation, OperationType};

        let entity: String = table
            .get("entity")
            .context("Operation must have 'entity' field")?;

        let op_str: String = table
            .get("operation")
            .context("Operation must have 'operation' field")?;

        let operation = OperationType::from_str(&op_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid operation type: {}", op_str))?;

        // Parse optional id
        let id = if let Ok(id_str) = table.get::<String>("id") {
            Some(uuid::Uuid::parse_str(&id_str).context("Invalid UUID in 'id' field")?)
        } else {
            None
        };

        // Parse fields
        let mut fields = std::collections::HashMap::new();
        if let Ok(fields_table) = table.get::<Table>("fields") {
            for pair in fields_table.pairs::<String, Value>() {
                let (key, value) = pair?;
                fields.insert(key, self.lua_to_json(value)?);
            }
        }

        // Parse reason (for skip)
        let reason = table.get::<String>("reason").ok();

        // Parse error (for error operation)
        let error = table.get::<String>("error").ok();

        Ok(LuaOperation {
            entity,
            operation,
            id,
            fields,
            reason,
            error,
        })
    }

    /// Get the stdlib context (for accessing logs, status updates)
    pub fn context(&self) -> Arc<Mutex<StdlibContext>> {
        self.context.clone()
    }

    /// Clear the stdlib context (logs, status)
    pub fn clear_context(&self) {
        if let Ok(mut ctx) = self.context.lock() {
            ctx.logs.clear();
            ctx.status = None;
        }
    }

    /// Get access to the underlying Lua instance
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default Lua runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let runtime = LuaRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_load_minimal_script() {
        let runtime = LuaRuntime::new().unwrap();
        
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target) return {} end
            return M
        "#;
        
        let module = runtime.load_script(script);
        assert!(module.is_ok());
    }

    #[test]
    fn test_missing_declare_function() {
        let runtime = LuaRuntime::new().unwrap();
        
        let script = r#"
            local M = {}
            function M.transform(source, target) return {} end
            return M
        "#;
        
        let result = runtime.load_script(script);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("declare"));
    }

    #[test]
    fn test_missing_transform_function() {
        let runtime = LuaRuntime::new().unwrap();
        
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            return M
        "#;
        
        let result = runtime.load_script(script);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("transform"));
    }

    #[test]
    fn test_run_declare() {
        let runtime = LuaRuntime::new().unwrap();
        
        let script = r#"
            local M = {}
            function M.declare()
                return {
                    source = {
                        account = {
                            fields = { "accountid", "name", "revenue" },
                            filter = "statecode eq 0",
                            expand = { "primarycontactid" },
                            top = 1000
                        }
                    },
                    target = {
                        account = {
                            fields = { "accountid", "name" }
                        }
                    }
                }
            end
            function M.transform(source, target) return {} end
            return M
        "#;
        
        let module = runtime.load_script(script).unwrap();
        let declaration = runtime.run_declare(&module).unwrap();
        
        assert!(declaration.source.contains_key("account"));
        assert!(declaration.target.contains_key("account"));
        
        let source_account = &declaration.source["account"];
        assert_eq!(source_account.fields, vec!["accountid", "name", "revenue"]);
        assert_eq!(source_account.filter, Some("statecode eq 0".to_string()));
        assert_eq!(source_account.expand, vec!["primarycontactid"]);
        assert_eq!(source_account.top, Some(1000));
    }

    #[test]
    fn test_run_transform() {
        let runtime = LuaRuntime::new().unwrap();
        
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target)
                local ops = {}
                for _, account in ipairs(source.account or {}) do
                    local existing = lib.find(target.account or {}, "name", account.name)
                    if existing then
                        table.insert(ops, {
                            entity = "account",
                            operation = "update",
                            id = existing.accountid,
                            fields = { revenue = account.revenue }
                        })
                    else
                        table.insert(ops, {
                            entity = "account",
                            operation = "create",
                            fields = {
                                name = account.name,
                                revenue = account.revenue
                            }
                        })
                    end
                end
                return ops
            end
            return M
        "#;
        
        let module = runtime.load_script(script).unwrap();
        
        let source_data = serde_json::json!({
            "account": [
                { "accountid": "11111111-1111-1111-1111-111111111111", "name": "Acme Corp", "revenue": 1000000 },
                { "accountid": "22222222-2222-2222-2222-222222222222", "name": "New Company", "revenue": 50000 }
            ]
        });
        
        let target_data = serde_json::json!({
            "account": [
                { "accountid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "name": "Acme Corp", "revenue": 500000 }
            ]
        });
        
        let operations = runtime.run_transform(&module, &source_data, &target_data).unwrap();
        
        assert_eq!(operations.len(), 2);
        
        // First should be update (Acme Corp exists)
        assert_eq!(operations[0].entity, "account");
        assert_eq!(operations[0].operation, super::super::types::OperationType::Update);
        assert!(operations[0].id.is_some());
        
        // Second should be create (New Company doesn't exist)
        assert_eq!(operations[1].entity, "account");
        assert_eq!(operations[1].operation, super::super::types::OperationType::Create);
        assert!(operations[1].id.is_none());
    }

    #[test]
    fn test_json_roundtrip() {
        let runtime = LuaRuntime::new().unwrap();
        
        let original = serde_json::json!({
            "name": "Test",
            "value": 42,
            "nested": {
                "array": [1, 2, 3],
                "boolean": true
            }
        });
        
        let lua_value = runtime.json_to_lua(&original).unwrap();
        let result = runtime.lua_to_json(lua_value).unwrap();
        
        assert_eq!(original, result);
    }

    #[test]
    fn test_sandboxing() {
        let runtime = LuaRuntime::new().unwrap();
        
        // io should not be available
        let result: Value = runtime.lua().load("return io").eval().unwrap();
        assert!(matches!(result, Value::Nil), "io should not be available");
        
        // os should not be available
        let result: Value = runtime.lua().load("return os").eval().unwrap();
        assert!(matches!(result, Value::Nil), "os should not be available");
        
        // debug should not be available
        let result: Value = runtime.lua().load("return debug").eval().unwrap();
        assert!(matches!(result, Value::Nil), "debug should not be available");
        
        // package should not be available (no require)
        let result: Value = runtime.lua().load("return package").eval().unwrap();
        assert!(matches!(result, Value::Nil), "package should not be available");
    }
}
