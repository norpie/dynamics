//! Batch operations for executing multiple Operations together

use super::operation::{Operation, OperationResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A collection of operations that can be executed individually or as a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operations {
    operations: Vec<Operation>,
}

impl Operations {
    /// Create a new empty operations collection
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Create operations from a vector of operations
    pub fn from_operations(operations: Vec<Operation>) -> Self {
        Self { operations }
    }

    /// Add a single operation to the collection
    pub fn add(mut self, operation: Operation) -> Self {
        self.operations.push(operation);
        self
    }

    /// Add a create operation
    pub fn create(mut self, entity: impl Into<String>, data: Value) -> Self {
        self.operations.push(Operation::create(entity, data));
        self
    }

    /// Add an update operation
    pub fn update(mut self, entity: impl Into<String>, id: impl Into<String>, data: Value) -> Self {
        self.operations.push(Operation::update(entity, id, data));
        self
    }

    /// Add a delete operation
    pub fn delete(mut self, entity: impl Into<String>, id: impl Into<String>) -> Self {
        self.operations.push(Operation::delete(entity, id));
        self
    }

    /// Add an upsert operation
    pub fn upsert(
        mut self,
        entity: impl Into<String>,
        key_field: impl Into<String>,
        key_value: impl Into<String>,
        data: Value,
    ) -> Self {
        self.operations.push(Operation::upsert(entity, key_field, key_value, data));
        self
    }

    /// Get the number of operations in this collection
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get a reference to the operations vector
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Get operations excluding those at the specified indices
    /// Used for retrying only failed operations
    pub fn excluding_indices(&self, succeeded_indices: &[usize]) -> Vec<(usize, &Operation)> {
        self.operations
            .iter()
            .enumerate()
            .filter(|(idx, _)| !succeeded_indices.contains(idx))
            .collect()
    }

    /// Create a new Operations containing only items NOT at the specified indices
    pub fn without_indices(&self, succeeded_indices: &[usize]) -> Self {
        let ops: Vec<Operation> = self.operations
            .iter()
            .enumerate()
            .filter(|(idx, _)| !succeeded_indices.contains(idx))
            .map(|(_, op)| op.clone())
            .collect();
        Self::from_operations(ops)
    }

    /// Extend this collection with operations from another collection
    pub fn extend(mut self, other: Operations) -> Self {
        self.operations.extend(other.operations);
        self
    }

    /// Execute operations with smart strategy selection
    /// - Single operation: execute individually
    /// - Multiple operations: execute as batch
    pub async fn execute(&self, client: &crate::api::DynamicsClient, resilience: &crate::api::ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        client.execute_batch(&self.operations, resilience).await
    }

    /// Force individual execution (each operation as separate HTTP request)
    pub async fn execute_individual(&self, client: &crate::api::DynamicsClient, resilience: &crate::api::ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        let mut results = Vec::with_capacity(self.operations.len());

        for operation in &self.operations {
            let result = operation.execute(client, resilience).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Force batch execution (all operations in single HTTP request)
    pub async fn execute_batch(&self, client: &crate::api::DynamicsClient, resilience: &crate::api::ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        client.execute_batch(&self.operations, resilience).await
    }

    /// Execute operations in parallel (each operation as separate concurrent HTTP request)
    pub async fn execute_parallel(&self, client: &crate::api::DynamicsClient, resilience: &crate::api::ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        if self.operations.is_empty() {
            return Ok(Vec::new());
        }

        // Use tokio to execute operations in parallel
        let mut handles = Vec::new();

        for operation in &self.operations {
            let op_clone = operation.clone();
            let client_clone = client.clone(); // Assuming DynamicsClient implements Clone

            let resilience_clone = resilience.clone();
            let handle = tokio::spawn(async move {
                op_clone.execute(&client_clone, &resilience_clone).await
            });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            let result = handle.await??;
            results.push(result);
        }

        Ok(results)
    }
}

impl Default for Operations {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Operation> for Operations {
    fn from(operation: Operation) -> Self {
        Self::from_operations(vec![operation])
    }
}

impl From<Vec<Operation>> for Operations {
    fn from(operations: Vec<Operation>) -> Self {
        Self::from_operations(operations)
    }
}

impl IntoIterator for Operations {
    type Item = Operation;
    type IntoIter = std::vec::IntoIter<Operation>;

    fn into_iter(self) -> Self::IntoIter {
        self.operations.into_iter()
    }
}

impl<'a> IntoIterator for &'a Operations {
    type Item = &'a Operation;
    type IntoIter = std::slice::Iter<'a, Operation>;

    fn into_iter(self) -> Self::IntoIter {
        self.operations.iter()
    }
}