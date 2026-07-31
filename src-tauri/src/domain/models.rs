use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolId {
    Npm,
    Maven,
    FlutterPub,
    Go,
    Cargo,
    Docker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    Read,
    Plan,
    Apply,
    Rollback,
    HealthCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDetection {
    pub tool: ToolId,
    pub installed: bool,
    pub version: Option<String>,
    pub capabilities: Vec<ToolCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionContext {
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolContext {
    pub project_directory: Option<String>,
    pub include_project_sources: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigScope {
    System,
    User,
    Project,
    Environment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSource {
    pub scope: ConfigScope,
    pub location: String,
    pub priority: u32,
    pub sensitive: bool,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveValue {
    pub value: Option<String>,
    pub sources: Vec<ConfigSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveConfig {
    pub tool: ToolId,
    pub values: BTreeMap<String, EffectiveValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadResult {
    pub effective_config: EffectiveConfig,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NonSensitiveValue(String);

impl NonSensitiveValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub values: BTreeMap<String, NonSensitiveValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedChange {
    pub file: String,
    pub field: String,
    pub previous_value: Option<String>,
    pub next_value: Option<String>,
    pub risk: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangePlan {
    pub id: String,
    pub tool: ToolId,
    #[serde(default)]
    pub target_checksums: BTreeMap<String, String>,
    pub file_checksums: BTreeMap<String, String>,
    pub changes: Vec<PlannedChange>,
}

impl ChangePlan {
    pub fn confirm(self, confirmed_at_ms: i64) -> ConfirmedChangePlan {
        ConfirmedChangePlan {
            plan: self,
            confirmed_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedChangePlan {
    plan: ChangePlan,
    pub confirmed_at_ms: i64,
}

impl ConfirmedChangePlan {
    pub fn plan(&self) -> &ChangePlan {
        &self.plan
    }

    pub fn into_plan(self) -> ChangePlan {
        self.plan
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRef(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub path: String,
    pub checksum: String,
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: SnapshotRef,
    pub created_at_ms: i64,
    pub files: Vec<SnapshotFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Scan,
    Apply,
    Rollback,
    HealthCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationOutcome {
    Succeeded,
    PartiallySucceeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub kind: OperationKind,
    pub tool: ToolId,
    pub outcome: OperationOutcome,
    pub snapshot: Option<SnapshotRef>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyResult {
    pub operation: Operation,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckTarget {
    pub address: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheckStatus {
    Healthy,
    DnsFailure,
    TlsFailure,
    AuthenticationFailure,
    Timeout,
    HttpFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub target: HealthCheckTarget,
    pub status: HealthCheckStatus,
    pub elapsed_ms: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterErrorCode {
    InvalidInput,
    ParseFailure,
    ExternalModification,
    Unsupported,
    IoFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterError {
    pub code: AdapterErrorCode,
    pub message: String,
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AdapterError {}
