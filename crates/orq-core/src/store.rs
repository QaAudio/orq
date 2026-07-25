use crate::db::{self, params, Connection, OptionalExtension};
use crate::error::{OrqError, Result};
use crate::types::*;
use chrono::{Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LOCAL_PRAGMAS: &str = r#"
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;
PRAGMA synchronous=NORMAL;
"#;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workspaces (
    name TEXT PRIMARY KEY,
    root TEXT NOT NULL,
    created_at TEXT NOT NULL,
    max_concurrent INTEGER NOT NULL DEFAULT 8,
    max_spawns_per_hour INTEGER NOT NULL DEFAULT 120
);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace TEXT NOT NULL,
    session TEXT,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_ws ON events(workspace, id);

CREATE TABLE IF NOT EXISTS poi_tables (
    workspace TEXT NOT NULL,
    name TEXT NOT NULL,
    table_type TEXT NOT NULL DEFAULT 'generic',
    columns_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (workspace, name)
);

CREATE TABLE IF NOT EXISTS pois (
    workspace TEXT NOT NULL,
    table_name TEXT NOT NULL,
    key TEXT NOT NULL,
    session TEXT,
    value_json TEXT NOT NULL,
    columns_json TEXT NOT NULL DEFAULT '{}',
    state TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 1,
    tier TEXT NOT NULL DEFAULT 'durable',
    blocked INTEGER NOT NULL DEFAULT 0,
    blocker_reason TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workspace, table_name, key)
);
CREATE INDEX IF NOT EXISTS idx_pois_session ON pois(workspace, session);

CREATE TABLE IF NOT EXISTS leases (
    id TEXT PRIMARY KEY,
    workspace TEXT NOT NULL,
    table_name TEXT NOT NULL,
    key TEXT NOT NULL,
    kind TEXT NOT NULL,
    holder TEXT NOT NULL,
    reason TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_leases_poi ON leases(workspace, table_name, key);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    workspace TEXT NOT NULL,
    session TEXT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    command TEXT NOT NULL,
    cwd TEXT,
    profile TEXT NOT NULL DEFAULT 'shell',
    claims_json TEXT NOT NULL DEFAULT '[]',
    depends_on_json TEXT NOT NULL DEFAULT '[]',
    needs_poi_json TEXT NOT NULL DEFAULT '[]',
    restart TEXT NOT NULL DEFAULT 'never',
    attempt INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 1,
    pid INTEGER,
    exit_code INTEGER,
    log_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tasks_ws ON tasks(workspace, status);

CREATE TABLE IF NOT EXISTS triggers (
    id TEXT PRIMARY KEY,
    workspace TEXT NOT NULL,
    name TEXT NOT NULL,
    event_pattern TEXT NOT NULL,
    condition TEXT,
    actions_json TEXT NOT NULL,
    blocking INTEGER NOT NULL DEFAULT 0,
    max_fires_per_hour INTEGER NOT NULL DEFAULT 60,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS trigger_fires (
    trigger_id TEXT NOT NULL,
    fired_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_fires ON trigger_fires(trigger_id, fired_at);

CREATE TABLE IF NOT EXISTS spawn_log (
    workspace TEXT NOT NULL,
    spawned_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS models (
    workspace TEXT NOT NULL,
    id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    capabilities_json TEXT NOT NULL DEFAULT '[]',
    cost_weight REAL NOT NULL DEFAULT 1.0,
    latency_weight REAL NOT NULL DEFAULT 1.0,
    recipe_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (workspace, id)
);

CREATE TABLE IF NOT EXISTS affinities (
    workspace TEXT NOT NULL,
    class TEXT NOT NULL,
    model_id TEXT NOT NULL,
    score REAL NOT NULL DEFAULT 0.5,
    confidence REAL NOT NULL DEFAULT 0.0,
    n INTEGER NOT NULL DEFAULT 0,
    epoch INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workspace, class, model_id)
);

CREATE TABLE IF NOT EXISTS affinity_epochs (
    workspace TEXT PRIMARY KEY,
    epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    workspace TEXT NOT NULL,
    session TEXT,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    class TEXT NOT NULL,
    strategy TEXT NOT NULL,
    policy TEXT NOT NULL,
    status TEXT NOT NULL,
    seed INTEGER NOT NULL,
    epoch INTEGER NOT NULL,
    k INTEGER NOT NULL DEFAULT 1,
    moa_layers INTEGER NOT NULL DEFAULT 1,
    aggregator_model TEXT,
    claims_json TEXT NOT NULL DEFAULT '[]',
    features_json TEXT NOT NULL DEFAULT '{}',
    route_reason TEXT NOT NULL DEFAULT '',
    winner_task_id TEXT,
    current_layer INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_jobs_ws ON jobs(workspace, status);

CREATE TABLE IF NOT EXISTS job_children (
    job_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'worker',
    layer INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (job_id, task_id)
);

CREATE TABLE IF NOT EXISTS eval_hooks (
    workspace TEXT PRIMARY KEY,
    command TEXT NOT NULL
);
"#;

#[derive(Debug)]
pub struct Store {
    conn: Mutex<Connection>,
    pub data_dir: PathBuf,
}

impl Store {
    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| OrqError::Other(format!("db lock: {e}")))
    }

    /// Run `f` inside a single transaction (BEGIN IMMEDIATE locally).
    /// The connection mutex is held for the whole transaction, so `f` must
    /// only use the `_conn` helper variants — never `self.*` store methods.
    fn with_tx<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn()?;
        conn.begin()?;
        match f(&conn) {
            Ok(v) => {
                conn.commit()?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.rollback();
                Err(e)
            }
        }
    }

    /// Local SQLite file under `data_dir` (default, offline-first).
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = Self::prepare_dirs(data_dir)?;
        let db_path = data_dir.join("orq.db");
        let conn = Connection::open_local(&db_path)?;
        conn.execute_batch(LOCAL_PRAGMAS)?;
        Self::init(conn, data_dir)
    }

    /// Remote libSQL/Turso primary over HTTP (hrana).
    pub fn open_remote(data_dir: impl AsRef<Path>, url: &str, token: &str) -> Result<Self> {
        let data_dir = Self::prepare_dirs(data_dir)?;
        let conn = Connection::open_remote(url, token)?;
        Self::init(conn, data_dir)
    }

    /// Mode selection from env: `ORQ_DB_URL` + `ORQ_DB_TOKEN` -> remote, else local.
    pub fn open_env(data_dir: impl AsRef<Path>) -> Result<Self> {
        match std::env::var("ORQ_DB_URL") {
            Ok(url) if !url.trim().is_empty() => {
                let token = std::env::var("ORQ_DB_TOKEN").unwrap_or_default();
                Self::open_remote(data_dir, url.trim(), token.trim())
            }
            _ => Self::open(data_dir),
        }
    }

    fn prepare_dirs(data_dir: impl AsRef<Path>) -> Result<PathBuf> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(data_dir.join("logs"))?;
        std::fs::create_dir_all(data_dir.join("tmp"))?;
        std::fs::create_dir_all(data_dir.join("snapshots"))?;
        Ok(data_dir)
    }

    fn init(conn: Connection, data_dir: PathBuf) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        migrate_tasks(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            data_dir,
        })
    }

    pub fn open_default() -> Result<Self> {
        let dir = default_data_dir()?;
        Self::open_env(dir)
    }

    /// True when backed by a remote libSQL primary.
    pub fn is_remote(&self) -> bool {
        self.conn().map(|c| c.is_remote()).unwrap_or(false)
    }

    /// Delete every row belonging to a workspace (cloud hygiene / test cleanup).
    pub fn drop_workspace(&self, name: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.begin()?;
        let res = (|| -> Result<()> {
            for sql in [
                "DELETE FROM events WHERE workspace=?1",
                "DELETE FROM pois WHERE workspace=?1",
                "DELETE FROM poi_tables WHERE workspace=?1",
                "DELETE FROM leases WHERE workspace=?1",
                "DELETE FROM tasks WHERE workspace=?1",
                "DELETE FROM triggers WHERE workspace=?1",
                "DELETE FROM spawn_log WHERE workspace=?1",
                "DELETE FROM models WHERE workspace=?1",
                "DELETE FROM affinities WHERE workspace=?1",
                "DELETE FROM affinity_epochs WHERE workspace=?1",
                "DELETE FROM jobs WHERE workspace=?1",
                "DELETE FROM eval_hooks WHERE workspace=?1",
                "DELETE FROM workspaces WHERE name=?1",
            ] {
                conn.execute(sql, params![name])?;
            }
            Ok(())
        })();
        match res {
            Ok(()) => conn.commit()?,
            Err(e) => {
                let _ = conn.rollback();
                return Err(e);
            }
        }
        Ok(())
    }

    pub fn ensure_workspace(&self, name: &str, root: Option<&str>) -> Result<Workspace> {
        if let Some(ws) = self.get_workspace(name)? {
            return Ok(ws);
        }
        let root = root
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.data_dir.join("workspaces").join(name).display().to_string());
        std::fs::create_dir_all(&root)?;
        let ws = Workspace {
            name: name.to_string(),
            root,
            created_at: now(),
            max_concurrent: 8,
            max_spawns_per_hour: 120,
        };
        self.conn()?.execute(
            "INSERT INTO workspaces (name, root, created_at, max_concurrent, max_spawns_per_hour) VALUES (?1,?2,?3,?4,?5)",
            params![
                ws.name,
                ws.root,
                ws.created_at.to_rfc3339(),
                ws.max_concurrent,
                ws.max_spawns_per_hour
            ],
        )?;
        // Built-in paths table
        self.create_poi_table(
            &ws.name,
            "paths",
            "paths",
            vec![
                ColumnDef {
                    name: "glob".into(),
                    col_type: "string".into(),
                    poi: true,
                },
                ColumnDef {
                    name: "holder".into(),
                    col_type: "string".into(),
                    poi: false,
                },
            ],
        )?;
        self.append_event(
            &ws.name,
            None,
            "workspace.created",
            json!({ "name": ws.name, "root": ws.root }),
        )?;
        Ok(ws)
    }

    pub fn get_workspace(&self, name: &str) -> Result<Option<Workspace>> {
        self.conn()?.query_row(
                "SELECT name, root, created_at, max_concurrent, max_spawns_per_hour FROM workspaces WHERE name=?1",
                params![name],
                |r| {
                    Ok(Workspace {
                        name: r.get(0)?,
                        root: r.get(1)?,
                        created_at: parse_dt(&r.get::<_, String>(2)?),
                        max_concurrent: r.get(3)?,
                        max_spawns_per_hour: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT name, root, created_at, max_concurrent, max_spawns_per_hour FROM workspaces ORDER BY name",
        )?;
        let rows = stmt.query_map((), |r| {
            Ok(Workspace {
                name: r.get(0)?,
                root: r.get(1)?,
                created_at: parse_dt(&r.get::<_, String>(2)?),
                max_concurrent: r.get(3)?,
                max_spawns_per_hour: r.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn set_workspace_budgets(
        &self,
        name: &str,
        max_concurrent: Option<u32>,
        max_spawns_per_hour: Option<u32>,
    ) -> Result<()> {
        let mut ws = self
            .get_workspace(name)?
            .ok_or_else(|| OrqError::WorkspaceNotFound(name.into()))?;
        if let Some(v) = max_concurrent {
            ws.max_concurrent = v;
        }
        if let Some(v) = max_spawns_per_hour {
            ws.max_spawns_per_hour = v;
        }
        self.conn()?.execute(
            "UPDATE workspaces SET max_concurrent=?1, max_spawns_per_hour=?2 WHERE name=?3",
            params![ws.max_concurrent, ws.max_spawns_per_hour, name],
        )?;
        Ok(())
    }

    pub fn append_event(
        &self,
        workspace: &str,
        session: Option<&str>,
        kind: &str,
        payload: serde_json::Value,
    ) -> Result<i64> {
        let conn = self.conn()?;
        append_event_conn(&conn, workspace, session, kind, payload)
    }

    pub fn list_events(
        &self,
        workspace: &str,
        after_id: Option<i64>,
        limit: usize,
        session: Option<&str>,
    ) -> Result<Vec<OrqEvent>> {
        let limit = limit.min(500).max(1);
        let mut sql = String::from(
            "SELECT id, workspace, session, kind, payload, created_at FROM events WHERE workspace=?1",
        );
        if after_id.is_some() {
            sql.push_str(" AND id > ?2");
        }
        if session.is_some() {
            sql.push_str(if after_id.is_some() {
                " AND session = ?3"
            } else {
                " AND session = ?2"
            });
        }
        sql.push_str(" ORDER BY id ASC LIMIT ");
        sql.push_str(&limit.to_string());

        let conn = self.conn()?;

        let mut stmt = conn.prepare(&sql)?;
        let map_row = |r: &db::Row| -> db::Result<OrqEvent> {
            let payload: String = r.get(4)?;
            Ok(OrqEvent {
                id: r.get(0)?,
                workspace: r.get(1)?,
                session: r.get(2)?,
                kind: r.get(3)?,
                payload: serde_json::from_str(&payload).unwrap_or(json!({})),
                created_at: parse_dt(&r.get::<_, String>(5)?),
            })
        };

        let rows: Vec<OrqEvent> = match (after_id, session) {
            (Some(aid), Some(s)) => stmt
                .query_map(params![workspace, aid, s], map_row)?
                .filter_map(|r| r.ok())
                .collect(),
            (Some(aid), None) => stmt
                .query_map(params![workspace, aid], map_row)?
                .filter_map(|r| r.ok())
                .collect(),
            (None, Some(s)) => stmt
                .query_map(params![workspace, s], map_row)?
                .filter_map(|r| r.ok())
                .collect(),
            (None, None) => stmt
                .query_map(params![workspace], map_row)?
                .filter_map(|r| r.ok())
                .collect(),
        };
        Ok(rows)
    }

    /// Most-recent-first events matching an exact `kind` (dashboard snapshot
    /// use, e.g. `trigger.action_error`) — unlike `list_events`, which is
    /// oldest-first with a hard cap and can miss recent rows once a
    /// workspace has produced more than `limit` events overall.
    pub fn list_recent_events_by_kind(
        &self,
        workspace: &str,
        kind: &str,
        limit: usize,
    ) -> Result<Vec<OrqEvent>> {
        let limit = limit.clamp(1, 200);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace, session, kind, payload, created_at FROM events WHERE workspace=?1 AND kind=?2 ORDER BY id DESC LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![workspace, kind, limit as i64], |r| {
                let payload: String = r.get(4)?;
                Ok(OrqEvent {
                    id: r.get(0)?,
                    workspace: r.get(1)?,
                    session: r.get(2)?,
                    kind: r.get(3)?,
                    payload: serde_json::from_str(&payload).unwrap_or(json!({})),
                    created_at: parse_dt(&r.get::<_, String>(5)?),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn create_poi_table(
        &self,
        workspace: &str,
        name: &str,
        table_type: &str,
        columns: Vec<ColumnDef>,
    ) -> Result<PoiTable> {
        let t = PoiTable {
            workspace: workspace.into(),
            name: name.into(),
            table_type: table_type.into(),
            columns,
            created_at: now(),
        };
        self.conn()?.execute(
            "INSERT OR IGNORE INTO poi_tables (workspace, name, table_type, columns_json, created_at) VALUES (?1,?2,?3,?4,?5)",
            params![
                t.workspace,
                t.name,
                t.table_type,
                serde_json::to_string(&t.columns)?,
                t.created_at.to_rfc3339()
            ],
        )?;
        Ok(t)
    }

    pub fn get_poi_table(&self, workspace: &str, name: &str) -> Result<Option<PoiTable>> {
        let conn = self.conn()?;
        get_poi_table_conn(&conn, workspace, name)
    }

    pub fn list_poi_tables(&self, workspace: &str) -> Result<Vec<PoiTable>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT workspace, name, table_type, columns_json, created_at FROM poi_tables WHERE workspace=?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![workspace], |r| {
            let cols: String = r.get(3)?;
            Ok(PoiTable {
                workspace: r.get(0)?,
                name: r.get(1)?,
                table_type: r.get(2)?,
                columns: serde_json::from_str(&cols).unwrap_or_default(),
                created_at: parse_dt(&r.get::<_, String>(4)?),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_poi(&self, workspace: &str, table: &str, key: &str) -> Result<Option<Poi>> {
        self.purge_expired_leases()?;
        let conn = self.conn()?;
        get_poi_conn(&conn, workspace, table, key)
    }

    pub fn list_pois(
        &self,
        workspace: &str,
        table: &str,
        session: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Poi>> {
        let limit = limit.min(500).max(1);
        let conn = self.conn()?;
        let mut stmt = if session.is_some() {
            conn.prepare(
                "SELECT workspace, table_name, key, session, value_json, columns_json, state, version, tier, blocked, blocker_reason, updated_at FROM pois WHERE workspace=?1 AND table_name=?2 AND session=?3 ORDER BY key LIMIT ?4",
            )?
        } else {
            conn.prepare(
                "SELECT workspace, table_name, key, session, value_json, columns_json, state, version, tier, blocked, blocker_reason, updated_at FROM pois WHERE workspace=?1 AND table_name=?2 ORDER BY key LIMIT ?3",
            )?
        };
        let rows = if let Some(s) = session {
            stmt.query_map(params![workspace, table, s, limit as i64], |r| Ok(row_to_poi(r)))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map(params![workspace, table, limit as i64], |r| Ok(row_to_poi(r)))?
                .filter_map(|r| r.ok())
                .collect()
        };
        Ok(rows)
    }

    pub fn set_poi(
        &self,
        workspace: &str,
        table: &str,
        key: &str,
        value: serde_json::Value,
        columns: HashMap<String, serde_json::Value>,
        state: Option<&str>,
        tier: StorageTier,
        session: Option<&str>,
        if_version: Option<i64>,
        holder_check: Option<&str>,
    ) -> Result<Poi> {
        // Lease check + CAS read + upsert + event: one atomic transaction.
        let poi = self.with_tx(|conn| {
            purge_expired_leases_conn(conn)?;
            if let Some(lease) = active_lease_conn(conn, workspace, table, key)? {
                if lease.kind == LeaseKind::Write || lease.kind == LeaseKind::ReadBlock {
                    if holder_check.map(|h| h != lease.holder).unwrap_or(true) {
                        return Err(OrqError::LockHeld {
                            holder: lease.holder,
                            reason: lease.reason,
                        });
                    }
                }
            }

            if get_poi_table_conn(conn, workspace, table)?.is_none() {
                return Err(OrqError::TableNotFound(table.into()));
            }

            let existing = get_poi_conn(conn, workspace, table, key)?;
            let (version, blocked, blocker_reason) = if let Some(ref e) = existing {
                if let Some(expected) = if_version {
                    if e.version != expected {
                        return Err(OrqError::CasConflict {
                            expected,
                            actual: e.version,
                        });
                    }
                }
                (e.version + 1, e.blocked, e.blocker_reason.clone())
            } else {
                if let Some(expected) = if_version {
                    if expected != 0 {
                        return Err(OrqError::CasConflict {
                            expected,
                            actual: 0,
                        });
                    }
                }
                (1, false, None)
            };

            let state = state
                .map(|s| s.to_string())
                .or_else(|| existing.as_ref().map(|e| e.state.clone()))
                .unwrap_or_default();
            let updated = now();
            conn.execute(
                "INSERT INTO pois (workspace, table_name, key, session, value_json, columns_json, state, version, tier, blocked, blocker_reason, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(workspace, table_name, key) DO UPDATE SET
                   session=excluded.session, value_json=excluded.value_json, columns_json=excluded.columns_json,
                   state=excluded.state, version=excluded.version, tier=excluded.tier, updated_at=excluded.updated_at",
                params![
                    workspace,
                    table,
                    key,
                    session,
                    value.to_string(),
                    serde_json::to_string(&columns)?,
                    state,
                    version,
                    tier.as_str(),
                    blocked as i32,
                    blocker_reason,
                    updated.to_rfc3339()
                ],
            )?;

            let poi = get_poi_conn(conn, workspace, table, key)?
                .ok_or_else(|| OrqError::Other("poi missing after set".into()))?;

            append_event_conn(
                conn,
                workspace,
                session,
                "poi.changed",
                json!({
                    "table": table,
                    "key": key,
                    "version": poi.version,
                    "state": poi.state,
                    "value": poi.value,
                }),
            )?;
            Ok(poi)
        })?;

        if tier == StorageTier::Versioned {
            let _ = self.export_poi_snapshot(&poi);
        }
        Ok(poi)
    }

    pub fn set_poi_blocked(
        &self,
        workspace: &str,
        table: &str,
        key: &str,
        blocked: bool,
        reason: Option<&str>,
    ) -> Result<Poi> {
        let mut poi = self
            .get_poi(workspace, table, key)?
            .ok_or_else(|| OrqError::PoiNotFound {
                table: table.into(),
                key: key.into(),
            })?;
        poi.blocked = blocked;
        poi.blocker_reason = reason.map(|s| s.to_string());
        poi.version += 1;
        poi.updated_at = now();
        self.with_tx(|conn| {
            conn.execute(
                "UPDATE pois SET blocked=?1, blocker_reason=?2, version=?3, updated_at=?4 WHERE workspace=?5 AND table_name=?6 AND key=?7",
                params![
                    blocked as i32,
                    poi.blocker_reason,
                    poi.version,
                    poi.updated_at.to_rfc3339(),
                    workspace,
                    table,
                    key
                ],
            )?;
            append_event_conn(
                conn,
                workspace,
                poi.session.as_deref(),
                if blocked { "poi.blocked" } else { "poi.unblocked" },
                json!({ "table": table, "key": key, "reason": reason }),
            )?;
            Ok(())
        })?;
        Ok(poi)
    }

    pub fn acquire_lease(
        &self,
        workspace: &str,
        table: &str,
        key: &str,
        kind: LeaseKind,
        holder: &str,
        reason: &str,
        ttl_secs: i64,
    ) -> Result<Lease> {
        self.with_tx(|conn| {
            purge_expired_leases_conn(conn)?;
            if let Some(existing) = active_lease_conn(conn, workspace, table, key)? {
                if existing.holder != holder {
                    return Err(OrqError::LockHeld {
                        holder: existing.holder,
                        reason: existing.reason,
                    });
                }
                // renew
                let expires = Utc::now() + Duration::seconds(ttl_secs);
                conn.execute(
                    "UPDATE leases SET expires_at=?1, kind=?2, reason=?3 WHERE id=?4",
                    params![expires.to_rfc3339(), kind.as_str(), reason, existing.id],
                )?;
                return Ok(Lease {
                    id: existing.id,
                    workspace: workspace.into(),
                    table: table.into(),
                    key: key.into(),
                    kind,
                    holder: holder.into(),
                    reason: reason.into(),
                    expires_at: expires,
                });
            }
            let lease = Lease {
                id: new_id(),
                workspace: workspace.into(),
                table: table.into(),
                key: key.into(),
                kind,
                holder: holder.into(),
                reason: reason.into(),
                expires_at: Utc::now() + Duration::seconds(ttl_secs),
            };
            conn.execute(
                "INSERT INTO leases (id, workspace, table_name, key, kind, holder, reason, expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    lease.id,
                    lease.workspace,
                    lease.table,
                    lease.key,
                    lease.kind.as_str(),
                    lease.holder,
                    lease.reason,
                    lease.expires_at.to_rfc3339()
                ],
            )?;
            append_event_conn(
                conn,
                workspace,
                None,
                "poi.locked",
                json!({ "table": table, "key": key, "kind": kind.as_str(), "holder": holder }),
            )?;
            Ok(lease)
        })
    }

    pub fn release_lease(&self, workspace: &str, table: &str, key: &str, holder: &str) -> Result<()> {
        self.with_tx(|conn| {
            conn.execute(
                "DELETE FROM leases WHERE workspace=?1 AND table_name=?2 AND key=?3 AND holder=?4",
                params![workspace, table, key, holder],
            )?;
            append_event_conn(
                conn,
                workspace,
                None,
                "poi.unlocked",
                json!({ "table": table, "key": key, "holder": holder }),
            )?;
            Ok(())
        })
    }

    pub fn steal_lease(
        &self,
        workspace: &str,
        table: &str,
        key: &str,
        new_holder: &str,
        reason: &str,
        ttl_secs: i64,
    ) -> Result<Lease> {
        self.conn()?.execute(
            "DELETE FROM leases WHERE workspace=?1 AND table_name=?2 AND key=?3",
            params![workspace, table, key],
        )?;
        self.acquire_lease(
            workspace,
            table,
            key,
            LeaseKind::Write,
            new_holder,
            reason,
            ttl_secs,
        )
    }

    pub fn active_lease(&self, workspace: &str, table: &str, key: &str) -> Result<Option<Lease>> {
        let conn = self.conn()?;
        active_lease_conn(&conn, workspace, table, key)
    }

    pub fn purge_expired_leases(&self) -> Result<()> {
        let conn = self.conn()?;
        purge_expired_leases_conn(&conn)
    }

    pub fn release_leases_for_holder(&self, holder: &str) -> Result<()> {
        self.conn()?.execute("DELETE FROM leases WHERE holder=?1", params![holder])?;
        Ok(())
    }

    /// All non-expired leases for a workspace (dashboard snapshot use), newest
    /// expiry last so a UI can show soonest-to-expire first by reversing.
    pub fn list_leases(&self, workspace: &str) -> Result<Vec<Lease>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace, table_name, key, kind, holder, reason, expires_at FROM leases WHERE workspace=?1 AND expires_at > ?2 ORDER BY expires_at ASC",
        )?;
        let rows = stmt
            .query_map(params![workspace, Utc::now().to_rfc3339()], |r| {
                Ok(Lease {
                    id: r.get(0)?,
                    workspace: r.get(1)?,
                    table: r.get(2)?,
                    key: r.get(3)?,
                    kind: LeaseKind::parse(&r.get::<_, String>(4)?).unwrap_or(LeaseKind::Write),
                    holder: r.get(5)?,
                    reason: r.get(6)?,
                    expires_at: parse_dt(&r.get::<_, String>(7)?),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Blocked POI rows across every table in a workspace (dashboard snapshot
    /// use) — `pois` is a single physical table with a `table_name` column,
    /// so this is one query, not a fan-out over `list_poi_tables`.
    pub fn list_blocked_pois(&self, workspace: &str, limit: usize) -> Result<Vec<Poi>> {
        let limit = limit.clamp(1, 500);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT workspace, table_name, key, session, value_json, columns_json, state, version, tier, blocked, blocker_reason, updated_at FROM pois WHERE workspace=?1 AND blocked=1 ORDER BY updated_at DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![workspace, limit as i64], |r| Ok(row_to_poi(r)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn insert_task(&self, task: &Task) -> Result<()> {
        self.with_tx(|conn| {
            conn.execute(
            "INSERT INTO tasks (id, workspace, session, name, kind, status, command, cwd, profile, claims_json, depends_on_json, needs_poi_json, restart, attempt, max_attempts, pid, exit_code, log_path, job_id, model_id, class, role, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)",
            params![
                task.id,
                task.workspace,
                task.session,
                task.name,
                task.kind.as_str(),
                task.status.as_str(),
                task.command,
                task.cwd,
                task.profile,
                serde_json::to_string(&task.claims)?,
                serde_json::to_string(&task.depends_on)?,
                serde_json::to_string(&task.needs_poi)?,
                task.restart.as_str(),
                task.attempt,
                task.max_attempts,
                task.pid.map(|p| p as i64),
                task.exit_code,
                task.log_path,
                task.job_id,
                task.model_id,
                task.class,
                task.role,
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339()
            ],
        )?;
            append_event_conn(
                conn,
                &task.workspace,
                task.session.as_deref(),
                "task.created",
                json!({ "id": task.id, "name": task.name, "command": task.command, "model_id": task.model_id, "job_id": task.job_id }),
            )?;
            Ok(())
        })
    }

    pub fn update_task(&self, task: &Task) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tasks SET status=?1, attempt=?2, pid=?3, exit_code=?4, log_path=?5, updated_at=?6, restart=?7, max_attempts=?8, job_id=?9, model_id=?10, class=?11, role=?12 WHERE id=?13",
            params![
                task.status.as_str(),
                task.attempt,
                task.pid.map(|p| p as i64),
                task.exit_code,
                task.log_path,
                task.updated_at.to_rfc3339(),
                task.restart.as_str(),
                task.max_attempts,
                task.job_id,
                task.model_id,
                task.class,
                task.role,
                task.id
            ],
        )?;
        Ok(())
    }

    const TASK_SELECT: &'static str = "SELECT id, workspace, session, name, kind, status, command, cwd, profile, claims_json, depends_on_json, needs_poi_json, restart, attempt, max_attempts, pid, exit_code, log_path, created_at, updated_at, job_id, model_id, class, role FROM tasks";

    pub fn get_task(&self, id: &str) -> Result<Option<Task>> {
        let sql = format!("{} WHERE id=?1", Self::TASK_SELECT);
        self.conn()?
            .query_row(&sql, params![id], |r| Ok(row_to_task(r)))
            .optional()
            .map_err(Into::into)
    }

    pub fn list_tasks(
        &self,
        workspace: &str,
        session: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Task>> {
        let limit = limit.min(500).max(1);
        let conn = self.conn()?;
        let mut stmt = if session.is_some() {
            conn.prepare(&format!(
                "{} WHERE workspace=?1 AND session=?2 ORDER BY created_at DESC LIMIT ?3",
                Self::TASK_SELECT
            ))?
        } else {
            conn.prepare(&format!(
                "{} WHERE workspace=?1 ORDER BY created_at DESC LIMIT ?2",
                Self::TASK_SELECT
            ))?
        };
        let rows = if let Some(s) = session {
            stmt.query_map(params![workspace, s, limit as i64], |r| Ok(row_to_task(r)))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map(params![workspace, limit as i64], |r| Ok(row_to_task(r)))?
                .filter_map(|r| r.ok())
                .collect()
        };
        Ok(rows)
    }

    pub fn list_active_tasks(&self, workspace: &str) -> Result<Vec<Task>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "{} WHERE workspace=?1 AND status IN ('queued','starting','running','blocked','interrupting')",
            Self::TASK_SELECT
        ))?;
        let rows = stmt
            .query_map(params![workspace], |r| Ok(row_to_task(r)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn list_tasks_for_job(&self, job_id: &str) -> Result<Vec<Task>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "{} WHERE job_id=?1 ORDER BY created_at ASC",
            Self::TASK_SELECT
        ))?;
        let rows = stmt
            .query_map(params![job_id], |r| Ok(row_to_task(r)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn insert_trigger(&self, rule: &TriggerRule) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO triggers (id, workspace, name, event_pattern, condition, actions_json, blocking, max_fires_per_hour, enabled, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                rule.id,
                rule.workspace,
                rule.name,
                rule.event_pattern,
                rule.condition,
                serde_json::to_string(&rule.actions)?,
                rule.blocking as i32,
                rule.max_fires_per_hour,
                rule.enabled as i32,
                rule.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_trigger(&self, id: &str) -> Result<Option<TriggerRule>> {
        self.conn()?.query_row(
                "SELECT id, workspace, name, event_pattern, condition, actions_json, blocking, max_fires_per_hour, enabled, created_at FROM triggers WHERE id=?1",
                params![id],
                |r| Ok(row_to_trigger(r)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_triggers(&self, workspace: &str) -> Result<Vec<TriggerRule>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace, name, event_pattern, condition, actions_json, blocking, max_fires_per_hour, enabled, created_at FROM triggers WHERE workspace=?1 ORDER BY name",
        )?;
        let rows = stmt
            .query_map(params![workspace], |r| Ok(row_to_trigger(r)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn set_trigger_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.conn()?.execute(
            "UPDATE triggers SET enabled=?1 WHERE id=?2",
            params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn record_trigger_fire(&self, trigger_id: &str) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO trigger_fires (trigger_id, fired_at) VALUES (?1,?2)",
            params![trigger_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn trigger_fires_last_hour(&self, trigger_id: &str) -> Result<u32> {
        let since = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let count: i64 = self.conn()?.query_row(
            "SELECT COUNT(*) FROM trigger_fires WHERE trigger_id=?1 AND fired_at >= ?2",
            params![trigger_id, since],
            |r| r.get(0),
        )?;
        Ok(count as u32)
    }

    pub fn record_spawn(&self, workspace: &str) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO spawn_log (workspace, spawned_at) VALUES (?1,?2)",
            params![workspace, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn spawns_last_hour(&self, workspace: &str) -> Result<u32> {
        let since = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let count: i64 = self.conn()?.query_row(
            "SELECT COUNT(*) FROM spawn_log WHERE workspace=?1 AND spawned_at >= ?2",
            params![workspace, since],
            |r| r.get(0),
        )?;
        Ok(count as u32)
    }

    pub fn delete_poi(&self, workspace: &str, table: &str, key: &str) -> Result<bool> {
        self.with_tx(|conn| {
            let n = conn.execute(
                "DELETE FROM pois WHERE workspace=?1 AND table_name=?2 AND key=?3",
                params![workspace, table, key],
            )?;
            if n == 0 {
                return Ok(false);
            }
            append_event_conn(
                conn,
                workspace,
                None,
                "poi.deleted",
                json!({ "table": table, "key": key }),
            )?;
            Ok(true)
        })
    }

    pub fn gc(&self, workspace: &str, session: Option<&str>) -> Result<serde_json::Value> {
        let deleted_pois;
        let deleted_tasks;
        if let Some(s) = session {
            deleted_pois = self.conn()?.execute(
                "DELETE FROM pois WHERE workspace=?1 AND (session=?2 OR tier='ephemeral')",
                params![workspace, s],
            )? as i64;
            deleted_tasks = self.conn()?.execute(
                "DELETE FROM tasks WHERE workspace=?1 AND session=?2 AND status IN ('done','failed','cancelled','killed')",
                params![workspace, s],
            )? as i64;
        } else {
            deleted_pois = self.conn()?.execute(
                "DELETE FROM pois WHERE workspace=?1 AND tier='ephemeral'",
                params![workspace],
            )? as i64;
            deleted_tasks = self.conn()?.execute(
                "DELETE FROM tasks WHERE workspace=?1 AND status IN ('done','failed','cancelled','killed') AND kind='oneshot'",
                params![workspace],
            )? as i64;
        }
        self.purge_expired_leases()?;
        // clean old fires
        let cutoff = (Utc::now() - Duration::hours(24)).to_rfc3339();
        self.conn()?.execute("DELETE FROM trigger_fires WHERE fired_at < ?1", params![cutoff])?;
        self.conn()?.execute("DELETE FROM spawn_log WHERE spawned_at < ?1", params![cutoff])?;
        Ok(json!({ "deleted_pois": deleted_pois, "deleted_tasks": deleted_tasks }))
    }

    pub fn export_poi_snapshot(&self, poi: &Poi) -> Result<PathBuf> {
        let dir = self
            .data_dir
            .join("snapshots")
            .join(&poi.workspace)
            .join(&poi.table);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.v{}.json", sanitize(&poi.key), poi.version));
        std::fs::write(&path, serde_json::to_string_pretty(poi)?)?;
        Ok(path)
    }

    pub fn export_workspace_snapshot(&self, workspace: &str) -> Result<PathBuf> {
        let dir = self.data_dir.join("snapshots").join(workspace);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("workspace-{}.json", Utc::now().format("%Y%m%d%H%M%S")));
        let tables = self.list_poi_tables(workspace)?;
        let mut all_pois = Vec::new();
        for t in &tables {
            all_pois.extend(self.list_pois(workspace, &t.name, None, 500)?);
        }
        let tasks = self.list_tasks(workspace, None, 500)?;
        let triggers = self.list_triggers(workspace)?;
        let payload = json!({
            "workspace": workspace,
            "pois": all_pois,
            "tasks": tasks,
            "triggers": triggers,
        });
        std::fs::write(&path, serde_json::to_string_pretty(&payload)?)?;
        Ok(path)
    }

    pub fn log_path_for(&self, task_id: &str) -> PathBuf {
        self.data_dir.join("logs").join(format!("{task_id}.log"))
    }

    // --- models / affinities / jobs ---

    pub fn upsert_model(&self, model: &Model) -> Result<()> {
        self.with_tx(|conn| {
            conn.execute(
                "INSERT INTO models (workspace, id, display_name, capabilities_json, cost_weight, latency_weight, recipe_json, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(workspace, id) DO UPDATE SET
                   display_name=excluded.display_name,
                   capabilities_json=excluded.capabilities_json,
                   cost_weight=excluded.cost_weight,
                   latency_weight=excluded.latency_weight,
                   recipe_json=excluded.recipe_json",
                params![
                    model.workspace,
                    model.id,
                    model.display_name,
                    serde_json::to_string(&model.capabilities)?,
                    model.cost_weight,
                    model.latency_weight,
                    serde_json::to_string(&model.recipe)?,
                    model.created_at.to_rfc3339()
                ],
            )?;
            append_event_conn(
                conn,
                &model.workspace,
                None,
                "model.registered",
                json!({ "id": model.id, "capabilities": model.capabilities }),
            )?;
            Ok(())
        })
    }

    pub fn get_model(&self, workspace: &str, id: &str) -> Result<Option<Model>> {
        self.conn()?
            .query_row(
                "SELECT workspace, id, display_name, capabilities_json, cost_weight, latency_weight, recipe_json, created_at FROM models WHERE workspace=?1 AND id=?2",
                params![workspace, id],
                |r| Ok(row_to_model(r)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_models(&self, workspace: &str) -> Result<Vec<Model>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT workspace, id, display_name, capabilities_json, cost_weight, latency_weight, recipe_json, created_at FROM models WHERE workspace=?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![workspace], |r| Ok(row_to_model(r)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn get_affinity_epoch(&self, workspace: &str) -> Result<i64> {
        let v: Option<i64> = self
            .conn()?
            .query_row(
                "SELECT epoch FROM affinity_epochs WHERE workspace=?1",
                params![workspace],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v.unwrap_or(0))
    }

    pub fn bump_affinity_epoch(&self, workspace: &str) -> Result<i64> {
        self.with_tx(|conn| {
            let cur: Option<i64> = conn
                .query_row(
                    "SELECT epoch FROM affinity_epochs WHERE workspace=?1",
                    params![workspace],
                    |r| r.get(0),
                )
                .optional()?;
            let next = cur.unwrap_or(0) + 1;
            conn.execute(
                "INSERT INTO affinity_epochs (workspace, epoch) VALUES (?1,?2)
                 ON CONFLICT(workspace) DO UPDATE SET epoch=excluded.epoch",
                params![workspace, next],
            )?;
            append_event_conn(
                conn,
                workspace,
                None,
                "affinity.epoch",
                json!({ "epoch": next }),
            )?;
            Ok(next)
        })
    }

    pub fn upsert_affinity(&self, a: &AffinityScore) -> Result<()> {
        self.with_tx(|conn| {
            conn.execute(
                "INSERT INTO affinities (workspace, class, model_id, score, confidence, n, epoch, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(workspace, class, model_id) DO UPDATE SET
                   score=excluded.score, confidence=excluded.confidence, n=excluded.n,
                   epoch=excluded.epoch, updated_at=excluded.updated_at",
                params![
                    a.workspace,
                    a.class,
                    a.model_id,
                    a.score,
                    a.confidence,
                    a.n,
                    a.epoch,
                    a.updated_at.to_rfc3339()
                ],
            )?;
            append_event_conn(
                conn,
                &a.workspace,
                None,
                "affinity.updated",
                json!({
                    "class": a.class,
                    "model_id": a.model_id,
                    "score": a.score,
                    "n": a.n,
                    "epoch": a.epoch
                }),
            )?;
            Ok(())
        })
    }

    pub fn get_affinity(
        &self,
        workspace: &str,
        class: &str,
        model_id: &str,
    ) -> Result<Option<AffinityScore>> {
        self.conn()?
            .query_row(
                "SELECT workspace, class, model_id, score, confidence, n, epoch, updated_at FROM affinities WHERE workspace=?1 AND class=?2 AND model_id=?3",
                params![workspace, class, model_id],
                |r| Ok(row_to_affinity(r)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_affinities(
        &self,
        workspace: &str,
        class: Option<&str>,
    ) -> Result<Vec<AffinityScore>> {
        let conn = self.conn()?;
        if let Some(c) = class {
            let mut stmt = conn.prepare(
                "SELECT workspace, class, model_id, score, confidence, n, epoch, updated_at FROM affinities WHERE workspace=?1 AND class=?2 ORDER BY score DESC",
            )?;
            let rows = stmt
                .query_map(params![workspace, c], |r| Ok(row_to_affinity(r)))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        } else {
            let mut stmt = conn.prepare(
                "SELECT workspace, class, model_id, score, confidence, n, epoch, updated_at FROM affinities WHERE workspace=?1 ORDER BY class, score DESC",
            )?;
            let rows = stmt
                .query_map(params![workspace], |r| Ok(row_to_affinity(r)))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        }
    }

    pub fn insert_job(&self, job: &Job) -> Result<()> {
        self.with_tx(|conn| {
            conn.execute(
                "INSERT INTO jobs (id, workspace, session, name, command, class, strategy, policy, status, seed, epoch, k, moa_layers, aggregator_model, claims_json, features_json, route_reason, winner_task_id, current_layer, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
                params![
                    job.id,
                    job.workspace,
                    job.session,
                    job.name,
                    job.command,
                    job.class,
                    job.strategy.as_str(),
                    job.policy.as_str(),
                    job.status.as_str(),
                    job.seed as i64,
                    job.epoch,
                    job.k,
                    job.moa_layers,
                    job.aggregator_model,
                    serde_json::to_string(&job.claims)?,
                    job.features.to_string(),
                    job.route_reason,
                    job.winner_task_id,
                    job.current_layer,
                    job.created_at.to_rfc3339(),
                    job.updated_at.to_rfc3339()
                ],
            )?;
            append_event_conn(
                conn,
                &job.workspace,
                job.session.as_deref(),
                "job.planned",
                json!({
                    "id": job.id,
                    "strategy": job.strategy.as_str(),
                    "class": job.class,
                    "reason": job.route_reason
                }),
            )?;
            Ok(())
        })
    }

    pub fn update_job(&self, job: &Job) -> Result<()> {
        self.conn()?.execute(
            "UPDATE jobs SET status=?1, winner_task_id=?2, current_layer=?3, route_reason=?4, updated_at=?5 WHERE id=?6",
            params![
                job.status.as_str(),
                job.winner_task_id,
                job.current_layer,
                job.route_reason,
                job.updated_at.to_rfc3339(),
                job.id
            ],
        )?;
        Ok(())
    }

    pub fn get_job(&self, id: &str) -> Result<Option<Job>> {
        self.conn()?
            .query_row(
                "SELECT id, workspace, session, name, command, class, strategy, policy, status, seed, epoch, k, moa_layers, aggregator_model, claims_json, features_json, route_reason, winner_task_id, current_layer, created_at, updated_at FROM jobs WHERE id=?1",
                params![id],
                |r| Ok(row_to_job(r)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_jobs(&self, workspace: &str, limit: usize) -> Result<Vec<Job>> {
        let limit = limit.min(500).max(1);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace, session, name, command, class, strategy, policy, status, seed, epoch, k, moa_layers, aggregator_model, claims_json, features_json, route_reason, winner_task_id, current_layer, created_at, updated_at FROM jobs WHERE workspace=?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![workspace, limit as i64], |r| Ok(row_to_job(r)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn add_job_child(
        &self,
        job_id: &str,
        task_id: &str,
        role: &str,
        layer: u32,
    ) -> Result<()> {
        self.conn()?.execute(
            "INSERT OR IGNORE INTO job_children (job_id, task_id, role, layer) VALUES (?1,?2,?3,?4)",
            params![job_id, task_id, role, layer],
        )?;
        Ok(())
    }

    pub fn job_children(&self, job_id: &str) -> Result<Vec<(String, String, u32)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT task_id, role, layer FROM job_children WHERE job_id=?1 ORDER BY layer, task_id",
        )?;
        let rows = stmt
            .query_map(params![job_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)? as u32,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn set_eval_hook(&self, workspace: &str, command: &str) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO eval_hooks (workspace, command) VALUES (?1,?2)
             ON CONFLICT(workspace) DO UPDATE SET command=excluded.command",
            params![workspace, command],
        )?;
        Ok(())
    }

    pub fn get_eval_hook(&self, workspace: &str) -> Result<Option<String>> {
        self.conn()?
            .query_row(
                "SELECT command FROM eval_hooks WHERE workspace=?1",
                params![workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
}

// --- connection-level helpers (usable inside `with_tx` without re-locking) ---

fn append_event_conn(
    conn: &Connection,
    workspace: &str,
    session: Option<&str>,
    kind: &str,
    payload: serde_json::Value,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO events (workspace, session, kind, payload, created_at) VALUES (?1,?2,?3,?4,?5)",
        params![
            workspace,
            session,
            kind,
            payload.to_string(),
            now().to_rfc3339()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn get_poi_conn(conn: &Connection, workspace: &str, table: &str, key: &str) -> Result<Option<Poi>> {
    conn.query_row(
        "SELECT workspace, table_name, key, session, value_json, columns_json, state, version, tier, blocked, blocker_reason, updated_at FROM pois WHERE workspace=?1 AND table_name=?2 AND key=?3",
        params![workspace, table, key],
        |r| Ok(row_to_poi(r)),
    )
    .optional()
    .map_err(Into::into)
}

fn get_poi_table_conn(conn: &Connection, workspace: &str, name: &str) -> Result<Option<PoiTable>> {
    conn.query_row(
        "SELECT workspace, name, table_type, columns_json, created_at FROM poi_tables WHERE workspace=?1 AND name=?2",
        params![workspace, name],
        |r| {
            let cols: String = r.get(3)?;
            Ok(PoiTable {
                workspace: r.get(0)?,
                name: r.get(1)?,
                table_type: r.get(2)?,
                columns: serde_json::from_str(&cols).unwrap_or_default(),
                created_at: parse_dt(&r.get::<_, String>(4)?),
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn active_lease_conn(
    conn: &Connection,
    workspace: &str,
    table: &str,
    key: &str,
) -> Result<Option<Lease>> {
    conn.query_row(
        "SELECT id, workspace, table_name, key, kind, holder, reason, expires_at FROM leases WHERE workspace=?1 AND table_name=?2 AND key=?3 AND expires_at > ?4",
        params![workspace, table, key, Utc::now().to_rfc3339()],
        |r| {
            Ok(Lease {
                id: r.get(0)?,
                workspace: r.get(1)?,
                table: r.get(2)?,
                key: r.get(3)?,
                kind: LeaseKind::parse(&r.get::<_, String>(4)?).unwrap_or(LeaseKind::Write),
                holder: r.get(5)?,
                reason: r.get(6)?,
                expires_at: parse_dt(&r.get::<_, String>(7)?),
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn purge_expired_leases_conn(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM leases WHERE expires_at <= ?1",
        params![Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn row_to_poi(r: &db::Row<'_>) -> Poi {
    let value_json: String = r.get(4).unwrap_or_else(|_| "{}".into());
    let columns_json: String = r.get(5).unwrap_or_else(|_| "{}".into());
    let tier_s: String = r.get(8).unwrap_or_else(|_| "durable".into());
    let blocked: i32 = r.get(9).unwrap_or(0);
    Poi {
        workspace: r.get(0).unwrap_or_default(),
        table: r.get(1).unwrap_or_default(),
        key: r.get(2).unwrap_or_default(),
        session: r.get(3).ok(),
        value: serde_json::from_str(&value_json).unwrap_or(json!({})),
        columns: serde_json::from_str(&columns_json).unwrap_or_default(),
        state: r.get(6).unwrap_or_default(),
        version: r.get(7).unwrap_or(1),
        tier: StorageTier::parse(&tier_s).unwrap_or(StorageTier::Durable),
        blocked: blocked != 0,
        blocker_reason: r.get(10).ok(),
        updated_at: parse_dt(&r.get::<_, String>(11).unwrap_or_else(|_| now().to_rfc3339())),
    }
}

fn row_to_task(r: &db::Row<'_>) -> Task {
    let claims: String = r.get(9).unwrap_or_else(|_| "[]".into());
    let deps: String = r.get(10).unwrap_or_else(|_| "[]".into());
    let needs: String = r.get(11).unwrap_or_else(|_| "[]".into());
    let kind_s: String = r.get(4).unwrap_or_else(|_| "oneshot".into());
    let status_s: String = r.get(5).unwrap_or_else(|_| "queued".into());
    let restart_s: String = r.get(12).unwrap_or_else(|_| "never".into());
    let pid: Option<i64> = r.get(15).ok().flatten();
    Task {
        id: r.get(0).unwrap_or_default(),
        workspace: r.get(1).unwrap_or_default(),
        session: r.get(2).ok(),
        name: r.get(3).unwrap_or_default(),
        kind: TaskKind::parse(&kind_s).unwrap_or(TaskKind::Oneshot),
        status: TaskStatus::parse(&status_s).unwrap_or(TaskStatus::Queued),
        command: r.get(6).unwrap_or_default(),
        cwd: r.get(7).ok(),
        profile: r.get(8).unwrap_or_else(|_| "shell".into()),
        claims: serde_json::from_str(&claims).unwrap_or_default(),
        depends_on: serde_json::from_str(&deps).unwrap_or_default(),
        needs_poi: serde_json::from_str(&needs).unwrap_or_default(),
        restart: RestartPolicy::parse(&restart_s).unwrap_or(RestartPolicy::Never),
        attempt: r.get(13).unwrap_or(0),
        max_attempts: r.get(14).unwrap_or(1),
        pid: pid.map(|p| p as u32),
        exit_code: r.get(16).ok().flatten(),
        log_path: r.get(17).ok(),
        created_at: parse_dt(&r.get::<_, String>(18).unwrap_or_else(|_| now().to_rfc3339())),
        updated_at: parse_dt(&r.get::<_, String>(19).unwrap_or_else(|_| now().to_rfc3339())),
        job_id: r.get(20).ok(),
        model_id: r.get(21).ok(),
        class: r.get(22).ok(),
        role: r.get(23).ok(),
    }
}

fn row_to_model(r: &db::Row<'_>) -> Model {
    let caps: String = r.get(3).unwrap_or_else(|_| "[]".into());
    let recipe: String = r.get(6).unwrap_or_else(|_| "{}".into());
    Model {
        workspace: r.get(0).unwrap_or_default(),
        id: r.get(1).unwrap_or_default(),
        display_name: r.get(2).unwrap_or_default(),
        capabilities: serde_json::from_str(&caps).unwrap_or_default(),
        cost_weight: r.get(4).unwrap_or(1.0),
        latency_weight: r.get(5).unwrap_or(1.0),
        recipe: serde_json::from_str(&recipe).unwrap_or(LaunchRecipe {
            kind: LaunchKind::Cli,
            template: "{cmd}".into(),
            model_arg: None,
            env: HashMap::new(),
        }),
        created_at: parse_dt(&r.get::<_, String>(7).unwrap_or_else(|_| now().to_rfc3339())),
    }
}

fn row_to_affinity(r: &db::Row<'_>) -> AffinityScore {
    AffinityScore {
        workspace: r.get(0).unwrap_or_default(),
        class: r.get(1).unwrap_or_default(),
        model_id: r.get(2).unwrap_or_default(),
        score: r.get(3).unwrap_or(0.5),
        confidence: r.get(4).unwrap_or(0.0),
        n: r.get::<_, i64>(5).unwrap_or(0) as u32,
        epoch: r.get(6).unwrap_or(0),
        updated_at: parse_dt(&r.get::<_, String>(7).unwrap_or_else(|_| now().to_rfc3339())),
    }
}

fn row_to_job(r: &db::Row<'_>) -> Job {
    let claims: String = r.get(14).unwrap_or_else(|_| "[]".into());
    let features: String = r.get(15).unwrap_or_else(|_| "{}".into());
    let strat: String = r.get(6).unwrap_or_else(|_| "single".into());
    let policy: String = r.get(7).unwrap_or_else(|_| "sticky".into());
    let status: String = r.get(8).unwrap_or_else(|_| "planned".into());
    Job {
        id: r.get(0).unwrap_or_default(),
        workspace: r.get(1).unwrap_or_default(),
        session: r.get(2).ok(),
        name: r.get(3).unwrap_or_default(),
        command: r.get(4).unwrap_or_default(),
        class: r.get(5).unwrap_or_default(),
        strategy: ScheduleStrategy::parse(&strat).unwrap_or(ScheduleStrategy::Single),
        policy: RoutePolicy::parse(&policy).unwrap_or(RoutePolicy::Sticky),
        status: JobStatus::parse(&status).unwrap_or(JobStatus::Planned),
        seed: r.get::<_, i64>(9).unwrap_or(0) as u64,
        epoch: r.get(10).unwrap_or(0),
        k: r.get::<_, i64>(11).unwrap_or(1) as u32,
        moa_layers: r.get::<_, i64>(12).unwrap_or(1) as u32,
        aggregator_model: r.get(13).ok(),
        claims: serde_json::from_str(&claims).unwrap_or_default(),
        features: serde_json::from_str(&features).unwrap_or(json!({})),
        route_reason: r.get(16).unwrap_or_default(),
        winner_task_id: r.get(17).ok(),
        current_layer: r.get::<_, i64>(18).unwrap_or(0) as u32,
        created_at: parse_dt(&r.get::<_, String>(19).unwrap_or_else(|_| now().to_rfc3339())),
        updated_at: parse_dt(&r.get::<_, String>(20).unwrap_or_else(|_| now().to_rfc3339())),
    }
}

fn migrate_tasks(conn: &Connection) -> Result<()> {
    for col in ["job_id", "model_id", "class", "role"] {
        let sql = format!("ALTER TABLE tasks ADD COLUMN {col} TEXT");
        let _ = conn.execute(&sql, ());
    }
    Ok(())
}

fn row_to_trigger(r: &db::Row<'_>) -> TriggerRule {
    let actions: String = r.get(5).unwrap_or_else(|_| "[]".into());
    TriggerRule {
        id: r.get(0).unwrap_or_default(),
        workspace: r.get(1).unwrap_or_default(),
        name: r.get(2).unwrap_or_default(),
        event_pattern: r.get(3).unwrap_or_default(),
        condition: r.get(4).ok(),
        actions: serde_json::from_str(&actions).unwrap_or_default(),
        blocking: r.get::<_, i32>(6).unwrap_or(0) != 0,
        max_fires_per_hour: r.get(7).unwrap_or(60),
        enabled: r.get::<_, i32>(8).unwrap_or(1) != 0,
        created_at: parse_dt(&r.get::<_, String>(9).unwrap_or_else(|_| now().to_rfc3339())),
    }
}

fn parse_dt(s: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn default_data_dir() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("ORQ_DATA_DIR") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::data_local_dir()
        .ok_or_else(|| OrqError::Other("cannot resolve data dir".into()))?;
    Ok(base.join("orq"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn lazy_workspace_and_events() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let ws = store.ensure_workspace("default", None).unwrap();
        assert_eq!(ws.name, "default");
        let events = store.list_events("default", None, 10, None).unwrap();
        assert!(!events.is_empty());
    }
}
