//! Transform execution for Lua scripts
//!
//! Provides async execution of Lua transform scripts with progress callbacks
//! and cancellation support.

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::runtime::LuaRuntime;
use super::stdlib::{LogMessage, StatusUpdate};
use super::types::{Declaration, LuaOperation};

/// Status update sent during transform execution
#[derive(Debug, Clone)]
pub enum ExecutionUpdate {
    /// Status message from lib.status()
    Status(String),
    /// Progress update from lib.progress()
    Progress { current: usize, total: usize },
    /// Log message from lib.log()
    Log(String),
    /// Warning from lib.warn()
    Warn(String),
    /// Transform started
    Started,
    /// Transform completed successfully
    Completed { operation_count: usize },
    /// Transform failed
    Failed(String),
}

/// Context for transform execution
pub struct ExecutionContext {
    /// Channel to send status updates
    pub update_tx: mpsc::Sender<ExecutionUpdate>,
    /// Flag to signal cancellation
    pub cancel_flag: Arc<AtomicBool>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(update_tx: mpsc::Sender<ExecutionUpdate>, cancel_flag: Arc<AtomicBool>) -> Self {
        ExecutionContext {
            update_tx,
            cancel_flag,
        }
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    /// Send a status update (non-blocking)
    pub fn send_update(&self, update: ExecutionUpdate) {
        // Use try_send to avoid blocking if the receiver is slow
        let _ = self.update_tx.try_send(update);
    }
}

/// Result of transform execution
#[derive(Debug)]
pub struct ExecutionResult {
    /// Generated operations
    pub operations: Vec<LuaOperation>,
    /// Log messages captured during execution
    pub logs: Vec<LogMessage>,
    /// Whether execution was cancelled
    pub was_cancelled: bool,
}

/// Execute a Lua transform script
///
/// This function runs the transform in a blocking manner (Lua is not async).
/// For async execution, use `execute_transform_async`.
pub fn execute_transform(
    script: &str,
    source_data: &serde_json::Value,
    target_data: &serde_json::Value,
) -> Result<ExecutionResult> {
    let runtime = LuaRuntime::new().context("Failed to create Lua runtime")?;

    let module = runtime
        .load_script(script)
        .context("Failed to load script")?;

    let operations = runtime
        .run_transform(&module, source_data, target_data)
        .context("Failed to run transform")?;

    // Get captured logs
    let logs = runtime
        .context()
        .lock()
        .map(|ctx| ctx.logs.clone())
        .unwrap_or_default();

    Ok(ExecutionResult {
        operations,
        logs,
        was_cancelled: false,
    })
}

/// Execute a Lua transform script asynchronously with progress updates
///
/// Runs the Lua transform in a blocking tokio task and streams status updates
/// through the provided context.
pub async fn execute_transform_async(
    script: String,
    source_data: serde_json::Value,
    target_data: serde_json::Value,
    ctx: ExecutionContext,
) -> Result<ExecutionResult> {
    // Send started update
    ctx.send_update(ExecutionUpdate::Started);

    // Run in a blocking task since Lua is not async
    let cancel_flag = ctx.cancel_flag.clone();
    let update_tx = ctx.update_tx.clone();

    let result = tokio::task::spawn_blocking(move || {
        execute_transform_with_updates(&script, &source_data, &target_data, cancel_flag, update_tx)
    })
    .await
    .context("Transform task panicked")?;

    match &result {
        Ok(exec_result) => {
            ctx.send_update(ExecutionUpdate::Completed {
                operation_count: exec_result.operations.len(),
            });
        }
        Err(e) => {
            ctx.send_update(ExecutionUpdate::Failed(e.to_string()));
        }
    }

    result
}

/// Internal function that runs transform and sends updates
fn execute_transform_with_updates(
    script: &str,
    source_data: &serde_json::Value,
    target_data: &serde_json::Value,
    cancel_flag: Arc<AtomicBool>,
    update_tx: mpsc::Sender<ExecutionUpdate>,
) -> Result<ExecutionResult> {
    // Check for cancellation before starting
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(ExecutionResult {
            operations: Vec::new(),
            logs: Vec::new(),
            was_cancelled: true,
        });
    }

    let runtime = LuaRuntime::new().context("Failed to create Lua runtime")?;

    let module = runtime
        .load_script(script)
        .context("Failed to load script")?;

    // Check cancellation after loading
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(ExecutionResult {
            operations: Vec::new(),
            logs: Vec::new(),
            was_cancelled: true,
        });
    }

    // Run the transform
    let operations = runtime
        .run_transform(&module, source_data, target_data)
        .context("Failed to run transform")?;

    // Get captured logs and status updates
    let (logs, final_status) = {
        let ctx = runtime.context();
        let guard = ctx.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        (guard.logs.clone(), guard.status.clone())
    };

    // Send any captured log messages
    for log in &logs {
        let update = match log {
            LogMessage::Info(msg) => ExecutionUpdate::Log(msg.clone()),
            LogMessage::Warn(msg) => ExecutionUpdate::Warn(msg.clone()),
        };
        let _ = update_tx.try_send(update);
    }

    // Send final status if present
    if let Some(status) = final_status {
        let update = match status {
            StatusUpdate::Status(msg) => ExecutionUpdate::Status(msg),
            StatusUpdate::Progress { current, total } => {
                ExecutionUpdate::Progress { current, total }
            }
        };
        let _ = update_tx.try_send(update);
    }

    Ok(ExecutionResult {
        operations,
        logs,
        was_cancelled: false,
    })
}

/// Run only the declare phase of a script
pub fn run_declare(script: &str) -> Result<Declaration> {
    let runtime = LuaRuntime::new().context("Failed to create Lua runtime")?;

    let module = runtime
        .load_script(script)
        .context("Failed to load script")?;

    runtime
        .run_declare(&module)
        .context("Failed to run declare()")
}

/// Validate operations returned by a transform
pub fn validate_operations(operations: &[LuaOperation]) -> Vec<String> {
    let mut errors = Vec::new();

    for (i, op) in operations.iter().enumerate() {
        if let Err(e) = op.validate() {
            errors.push(format!("Operation {}: {}", i + 1, e));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_simple_transform() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target)
                return {
                    { entity = "account", operation = "create", fields = { name = "Test" } }
                }
            end
            return M
        "#;

        let source = serde_json::json!({});
        let target = serde_json::json!({});

        let result = execute_transform(script, &source, &target).unwrap();

        assert_eq!(result.operations.len(), 1);
        assert_eq!(result.operations[0].entity, "account");
        assert!(!result.was_cancelled);
    }

    #[test]
    fn test_execute_with_data() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target)
                local ops = {}
                for _, account in ipairs(source.account or {}) do
                    table.insert(ops, {
                        entity = "account",
                        operation = "create",
                        fields = { name = account.name }
                    })
                end
                return ops
            end
            return M
        "#;

        let source = serde_json::json!({
            "account": [
                { "name": "Account 1" },
                { "name": "Account 2" }
            ]
        });
        let target = serde_json::json!({});

        let result = execute_transform(script, &source, &target).unwrap();

        assert_eq!(result.operations.len(), 2);
    }

    #[test]
    fn test_execute_captures_logs() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target)
                lib.log("Processing started")
                lib.warn("This is a warning")
                return {}
            end
            return M
        "#;

        let result = execute_transform(script, &serde_json::json!({}), &serde_json::json!({})).unwrap();

        assert_eq!(result.logs.len(), 2);
        assert!(matches!(&result.logs[0], LogMessage::Info(s) if s == "Processing started"));
        assert!(matches!(&result.logs[1], LogMessage::Warn(s) if s == "This is a warning"));
    }

    #[test]
    fn test_run_declare() {
        let script = r#"
            local M = {}
            function M.declare()
                return {
                    source = {
                        account = { fields = { "name", "revenue" } }
                    },
                    target = {
                        account = { fields = { "name" } }
                    }
                }
            end
            function M.transform(source, target) return {} end
            return M
        "#;

        let declaration = run_declare(script).unwrap();

        assert!(declaration.source.contains_key("account"));
        assert!(declaration.target.contains_key("account"));
        assert_eq!(declaration.source["account"].fields.len(), 2);
    }

    #[test]
    fn test_validate_operations() {
        use super::super::types::OperationType;

        let operations = vec![
            LuaOperation {
                entity: "account".to_string(),
                operation: OperationType::Create,
                id: None,
                fields: std::collections::HashMap::new(), // Empty fields - invalid for create
                reason: None,
                error: None,
            },
            LuaOperation {
                entity: "account".to_string(),
                operation: OperationType::Update,
                id: None, // Missing id - invalid for update
                fields: [("name".to_string(), serde_json::json!("Test"))].into_iter().collect(),
                reason: None,
                error: None,
            },
        ];

        let errors = validate_operations(&operations);

        assert_eq!(errors.len(), 2);
    }

    #[tokio::test]
    async fn test_execute_async() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target)
                lib.status("Processing...")
                lib.progress(50, 100)
                return {
                    { entity = "account", operation = "create", fields = { name = "Test" } }
                }
            end
            return M
        "#.to_string();

        let (tx, mut rx) = mpsc::channel(100);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let ctx = ExecutionContext::new(tx, cancel_flag);

        let result = execute_transform_async(
            script,
            serde_json::json!({}),
            serde_json::json!({}),
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.operations.len(), 1);

        // Check that we received updates
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(update) = rx.try_recv() {
            match update {
                ExecutionUpdate::Started => received_started = true,
                ExecutionUpdate::Completed { .. } => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started);
        assert!(received_completed);
    }

    #[tokio::test]
    async fn test_cancellation() {
        let script = r#"
            local M = {}
            function M.declare() return { source = {}, target = {} } end
            function M.transform(source, target) return {} end
            return M
        "#.to_string();

        let (tx, _rx) = mpsc::channel(100);
        let cancel_flag = Arc::new(AtomicBool::new(true)); // Pre-cancelled
        let ctx = ExecutionContext::new(tx, cancel_flag);

        let result = execute_transform_async(
            script,
            serde_json::json!({}),
            serde_json::json!({}),
            ctx,
        )
        .await
        .unwrap();

        assert!(result.was_cancelled);
        assert!(result.operations.is_empty());
    }
}
