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

    Ok(json!({
        "updated": Utc::now().to_rfc3339(),
        "workspace": workspace,
        "board": board,
        "tasks": tasks,
        "jobs": jobs,
        "affinities": affinities,
        "events": events,
        "files": files,
    }))
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
            "events",
            "files",
        ] {
            assert!(snap.get(key).is_some(), "missing key {key}");
        }
        let board = snap["board"].as_array().unwrap();
        assert!(board.iter().any(|p| p["key"] == "alpha"));

        let out = dir.path().join("dash").join("data.json");
        write_snapshot_atomic(&out, &snap).unwrap();
        let loaded: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(loaded["workspace"], "default");
    }
}
