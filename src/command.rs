use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A command exposed by a database driver.
///
/// The manifest is intentionally driver-agnostic so callers such as Workflow
/// and the frontend do not need to know which commands a particular driver
/// implements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverCommandDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Result returned by a driver command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverCommandResult {
    pub data: Value,
    pub row_count: Option<u64>,
}

impl DriverCommandResult {
    pub fn new(data: Value) -> Self {
        Self {
            data,
            row_count: None,
        }
    }
}
