use crate::error::Result;
use crate::store::Store;
use chrono::Utc;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Build a dashboard JSON snapshot from the store.
pub fn build_snapshot(
    store: &Store,
    workspace: &str,
    session: Option<&str>,
    row_limit: usize,
) -> Result<Value> {
    let row_limit = row_limit.clamp(1, 500);
    let board = if store.get_poi_table(workspace, "board")?.is_some() {
        store.list_pois(workspace, "board", session, row_limit)?
    } else {
        let mut all = Vec::new();
        for table in store.list_poi_tables(workspace)? {
            let mut rows = store.list_pois(workspace, &table.name, session, 20)?;
            all.append(&mut rows);
            if all.len() >= row_limit {
                all.truncate(row_limit);
                break;
            }
        }
        all
    };

    let tasks = store.list_tasks(workspace, session, row_limit)?;
    let jobs = store.list_jobs(workspace, row_limit)?;
    let affinities = store.list_affinities(workspace, None)?;
    let events = store.list_events(workspace, None, row_limit.min(100), session)?;
    let files = list_workspace_files(store, workspace, 80)?;
    let canvases = list_canvases(store, workspace, session, row_limit)?;

    // Best-effort enrichment (G3): everything below is read from store APIs
    // that already exist for other commands — none of this is authoritative
    // state, it's display-only context for the dashboard (mirrors the same
    // "POIs are display, not authorization" posture as PORQ_LOOP_SCHEMA.md §9).
    let leases = store.list_leases(workspace).unwrap_or_default();
    let triggers = store.list_triggers(workspace).unwrap_or_default();
    let blocked_pois = store.list_blocked_pois(workspace, row_limit).unwrap_or_default();
    let models = store.list_models(workspace).unwrap_or_default();
    // Most recent trigger action failures, independent of the main `events`
    // feed's oldest-first cap (a workspace with >100 lifetime events would
    // otherwise never surface a fresh failure there).
    let trigger_failures = store
        .list_recent_events_by_kind(workspace, "trigger.action_error", 20)
        .unwrap_or_default();
    let active_sessions: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        for t in &tasks {
            if !t.status.is_terminal() {
                if let Some(s) = &t.session {
                    seen.insert(s.clone());
                }
            }
        }
        seen.into_iter().collect()
    };
    let daemon_running = crate::daemon::is_daemon_running(&store.data_dir);

    Ok(json!({
        "updated": Utc::now().to_rfc3339(),
        "workspace": workspace,
        "board": board,
        "tasks": tasks,
        "jobs": jobs,
        "affinities": affinities,
        "models": models,
        "events": events,
        "files": files,
        "canvases": canvases,
        "leases": leases,
        "triggers": triggers,
        "blocked_pois": blocked_pois,
        "trigger_failures": trigger_failures,
        "active_sessions": active_sessions,
        "daemon": {
            "running": daemon_running,
        },
    }))
}

fn list_canvases(
    store: &Store,
    workspace: &str,
    session: Option<&str>,
    row_limit: usize,
) -> Result<Vec<crate::types::Poi>> {
    if store.get_poi_table(workspace, "canvas")?.is_none() {
        return Ok(vec![]);
    }
    let mut rows = store.list_pois(workspace, "canvas", session, row_limit)?;
    rows.sort_by(|a, b| {
        let oa = a
            .columns
            .get("order")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(0);
        let ob = b
            .columns
            .get("order")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(0);
        oa.cmp(&ob).then_with(|| a.key.cmp(&b.key))
    });
    Ok(rows)
}

fn list_workspace_files(store: &Store, workspace: &str, limit: usize) -> Result<Vec<String>> {
    let Some(ws) = store.get_workspace(workspace)? else {
        return Ok(vec![]);
    };
    let root = PathBuf::from(&ws.root);
    if !root.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    walk_files(&root, &root, 0, 3, limit, &mut out);
    out.sort();
    Ok(out)
}

fn walk_files(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    limit: usize,
    out: &mut Vec<String>,
) {
    if out.len() >= limit || depth > max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= limit {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            walk_files(root, &path, depth + 1, max_depth, limit, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Default snapshot path under the data dir.
pub fn default_snapshot_path(data_dir: &Path) -> PathBuf {
    data_dir.join("dash").join("data.json")
}

/// Write JSON atomically (temp file + rename).
pub fn write_snapshot_atomic(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp, bytes)?;
    let _ = fs::remove_file(path);
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Build and write a snapshot in one call.
pub fn write_snapshot(
    store: &Store,
    workspace: &str,
    session: Option<&str>,
    out: &Path,
) -> Result<Value> {
    let snap = build_snapshot(store, workspace, session, 100)?;
    write_snapshot_atomic(out, &snap)?;
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ColumnDef, StorageTier};
    use tempfile::tempdir;

    #[test]
    fn snapshot_has_required_keys_after_seed() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.ensure_workspace("default", None).unwrap();
        store
            .create_poi_table(
                "default",
                "board",
                "generic",
                vec![ColumnDef {
                    name: "owner".into(),
                    col_type: "string".into(),
                    poi: false,
                }],
            )
            .unwrap();
        store
            .set_poi(
                "default",
                "board",
                "alpha",
                json!({"note": "e2e"}),
                Default::default(),
                Some("pending"),
                StorageTier::Ephemeral,
                None,
                None,
                None,
            )
            .unwrap();

        let snap = build_snapshot(&store, "default", None, 50).unwrap();
        for key in [
            "updated",
            "workspace",
            "board",
            "tasks",
            "jobs",
            "affinities",
            "models",
            "events",
            "files",
            "canvases",
            "leases",
            "triggers",
            "blocked_pois",
            "trigger_failures",
            "active_sessions",
            "daemon",
        ] {
            assert!(snap.get(key).is_some(), "missing key {key}");
        }
        assert!(snap["canvases"].as_array().unwrap().is_empty());
        let board = snap["board"].as_array().unwrap();
        assert!(board.iter().any(|p| p["key"] == "alpha"));
        assert!(snap["daemon"]["running"].is_boolean());
        assert!(snap["leases"].as_array().unwrap().is_empty());
        assert!(snap["blocked_pois"].as_array().unwrap().is_empty());

        let out = dir.path().join("dash").join("data.json");
        write_snapshot_atomic(&out, &snap).unwrap();
        let loaded: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(loaded["workspace"], "default");
    }

    #[test]
    fn snapshot_reports_blocked_pois_and_leases() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.ensure_workspace("default", None).unwrap();
        store
            .create_poi_table("default", "board", "generic", vec![])
            .unwrap();
        store
            .set_poi(
                "default",
                "board",
                "stuck",
                json!({"note": "blocked row"}),
                Default::default(),
                Some("pending"),
                StorageTier::Ephemeral,
                None,
                None,
                None,
            )
            .unwrap();
        store
            .set_poi_blocked("default", "board", "stuck", true, Some("waiting on human"))
            .unwrap();
        store
            .acquire_lease(
                "default",
                "board",
                "stuck",
                crate::types::LeaseKind::Write,
                "agent-1",
                "in progress",
                60,
            )
            .unwrap();

        let snap = build_snapshot(&store, "default", None, 50).unwrap();
        let blocked = snap["blocked_pois"].as_array().unwrap();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0]["key"], "stuck");
        let leases = snap["leases"].as_array().unwrap();
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0]["holder"], "agent-1");
    }
}
