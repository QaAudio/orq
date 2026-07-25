use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageTier {
    Ephemeral,
    Durable,
    Versioned,
}

impl StorageTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ephemeral => "ephemeral",
            Self::Durable => "durable",
            Self::Versioned => "versioned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ephemeral" => Some(Self::Ephemeral),
            "durable" => Some(Self::Durable),
            "versioned" => Some(Self::Versioned),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseKind {
    Write,
    ReadBlock,
}

impl LeaseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::ReadBlock => "read_block",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "write" => Some(Self::Write),
            "read_block" => Some(Self::ReadBlock),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Oneshot,
    Service,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oneshot => "oneshot",
            Self::Service => "service",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "oneshot" => Some(Self::Oneshot),
            "service" => Some(Self::Service),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

impl RestartPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::OnFailure => "on_failure",
            Self::Always => "always",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "never" => Some(Self::Never),
            "on_failure" => Some(Self::OnFailure),
            "always" => Some(Self::Always),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Starting,
    Running,
    Blocked,
    Interrupting,
    Done,
    Failed,
    Cancelled,
    Killed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Interrupting => "interrupting",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Killed => "killed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "starting" => Some(Self::Starting),
            "running" => Some(Self::Running),
            "blocked" => Some(Self::Blocked),
            "interrupting" => Some(Self::Interrupting),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "killed" => Some(Self::Killed),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed | Self::Cancelled | Self::Killed
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub root: String,
    pub created_at: DateTime<Utc>,
    pub max_concurrent: u32,
    pub max_spawns_per_hour: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrqEvent {
    pub id: i64,
    pub workspace: String,
    pub session: Option<String>,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoiTable {
    pub workspace: String,
    pub name: String,
    pub table_type: String,
    pub columns: Vec<ColumnDef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: String,
    #[serde(default)]
    pub poi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poi {
    pub workspace: String,
    pub table: String,
    pub key: String,
    pub session: Option<String>,
    pub value: serde_json::Value,
    pub columns: HashMap<String, serde_json::Value>,
    pub state: String,
    pub version: i64,
    pub tier: StorageTier,
    pub blocked: bool,
    pub blocker_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub id: String,
    pub workspace: String,
    pub table: String,
    pub key: String,
    pub kind: LeaseKind,
    pub holder: String,
    pub reason: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub workspace: String,
    pub session: Option<String>,
    pub name: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub command: String,
    pub cwd: Option<String>,
    pub profile: String,
    pub claims: Vec<String>,
    pub depends_on: Vec<String>,
    pub needs_poi: Vec<String>,
    pub restart: RestartPolicy,
    pub attempt: u32,
    pub max_attempts: u32,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub log_path: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchKind {
    Cli,
    HttpStub,
}

impl LaunchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::HttpStub => "http_stub",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cli" => Some(Self::Cli),
            "http_stub" => Some(Self::HttpStub),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRecipe {
    pub kind: LaunchKind,
    /// CLI template with {cmd}, {model}, {prompt_file}
    pub template: String,
    #[serde(default)]
    pub model_arg: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub workspace: String,
    pub id: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
    pub cost_weight: f64,
    pub latency_weight: f64,
    pub recipe: LaunchRecipe,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityScore {
    pub workspace: String,
    pub class: String,
    pub model_id: String,
    pub score: f64,
    pub confidence: f64,
    pub n: u32,
    pub epoch: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleStrategy {
    Single,
    Race,
    Moa,
}

impl ScheduleStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Race => "race",
            Self::Moa => "moa",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "single" => Some(Self::Single),
            "race" => Some(Self::Race),
            "moa" => Some(Self::Moa),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutePolicy {
    Sticky,
    Epsilon,
    Softmax,
}

impl RoutePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sticky => "sticky",
            Self::Epsilon => "epsilon",
            Self::Softmax => "softmax",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sticky" => Some(Self::Sticky),
            "epsilon" => Some(Self::Epsilon),
            "softmax" => Some(Self::Softmax),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Planned,
    Running,
    Reconciling,
    Done,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Reconciling => "reconciling",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "planned" => Some(Self::Planned),
            "running" => Some(Self::Running),
            "reconciling" => Some(Self::Reconciling),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub workspace: String,
    pub session: Option<String>,
    pub name: String,
    pub command: String,
    pub class: String,
    pub strategy: ScheduleStrategy,
    pub policy: RoutePolicy,
    pub status: JobStatus,
    pub seed: u64,
    pub epoch: i64,
    pub k: u32,
    pub moa_layers: u32,
    pub aggregator_model: Option<String>,
    pub claims: Vec<String>,
    pub features: serde_json::Value,
    pub route_reason: String,
    pub winner_task_id: Option<String>,
    pub current_layer: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub class: String,
    pub features: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub class: String,
    pub models: Vec<String>,
    pub aggregator: Option<String>,
    pub policy: RoutePolicy,
    pub seed: u64,
    pub epoch: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    pub id: String,
    pub workspace: String,
    pub name: String,
    pub event_pattern: String,
    pub condition: Option<String>,
    pub actions: Vec<TriggerAction>,
    pub blocking: bool,
    pub max_fires_per_hour: u32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TriggerAction {
    SpawnTask {
        command: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        kind: Option<String>,
    },
    CancelTasks {
        selector: String,
    },
    SetPoi {
        table: String,
        key: String,
        value: serde_json::Value,
        #[serde(default)]
        state: Option<String>,
    },
    RunHook {
        command: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterProfile {
    pub name: String,
    pub command_template: String,
    pub readiness: Option<String>,
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn now() -> DateTime<Utc> {
    Utc::now()
}
