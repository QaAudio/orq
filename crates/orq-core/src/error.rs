use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrqError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),
    #[error("poi table not found: {0}")]
    TableNotFound(String),
    #[error("poi not found: {table}/{key}")]
    PoiNotFound { table: String, key: String },
    #[error("CAS conflict: expected version {expected}, got {actual}")]
    CasConflict { expected: i64, actual: i64 },
    #[error("lock held by {holder}: {reason}")]
    LockHeld { holder: String, reason: String },
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("trigger not found: {0}")]
    TriggerNotFound(String),
    #[error("trigger veto: {0}")]
    TriggerVeto(String),
    #[error("trigger cascade depth exceeded ({0})")]
    CascadeDepth(u32),
    #[error("trigger budget exceeded: {0}")]
    BudgetExceeded(String),
    #[error("claim overlap with running task {0}")]
    ClaimOverlap(String),
    #[error("daemon error: {0}")]
    Daemon(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite: {0}")]
    Sqlite(#[from] crate::db::DbError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, OrqError>;
