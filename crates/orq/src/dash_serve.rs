use anyhow::{bail, Context, Result};
use orq_core::dash::{default_snapshot_path, write_snapshot};
use orq_core::Store;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Hard cap on `tailBytes` regardless of what the client asks for, so a
/// crafted query can't force the server to buffer an unbounded log file.
const MAX_LOG_TAIL_BYTES: u64 = 256 * 1024;
const DEFAULT_LOG_TAIL_BYTES: u64 = 8 * 1024;

pub fn resolve_dash_root(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    if let Ok(p) = std::env::var("ORQ_DASH_ROOT") {
        return PathBuf::from(p);
    }
    let from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dashboard");
    if from_manifest.join("index.html").is_file() {
        return from_manifest;
    }
    PathBuf::from("web/dashboard")
}

pub fn run_serve(
    store: Arc<Store>,
    workspace: String,
    session: Option<String>,
    data_dir: PathBuf,
    port: u16,
    root: PathBuf,
    snapshot_out: Option<PathBuf>,
) -> Result<()> {
    if !root.join("index.html").is_file() {
        bail!(
            "dashboard root missing index.html: {} (set ORQ_DASH_ROOT or --root)",
            root.display()
        );
    }
    let snap_path = snapshot_out.unwrap_or_else(|| default_snapshot_path(&data_dir));
    write_snapshot(
        &store,
        &workspace,
        session.as_deref(),
        &snap_path,
    )
    .context("initial snapshot")?;

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        let store = store.clone();
        let workspace = workspace.clone();
        let session = session.clone();
        let snap_path = snap_path.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));
                let _ = write_snapshot(&store, &workspace, session.as_deref(), &snap_path);
            }
        });
    }

    let canvas_dir = data_dir.join("canvas");
    let _ = fs::create_dir_all(&canvas_dir);

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).with_context(|| format!("bind {addr}"))?;
    eprintln!(
        "porq dash serve http://{addr}/ (root={}, data={})",
        root.display(),
        snap_path.display()
    );

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let req = String::from_utf8_lossy(&buf[..n]);
        let raw_path = req
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string();
        let path = raw_path.split('?').next().unwrap_or("/");

        let (status, ctype, body) = if path == "/" || path == "/index.html" {
            file_response(&root.join("index.html"), "text/html; charset=utf-8")
        } else if path == "/app.js" {
            file_response(
                &root.join("app.js"),
                "application/javascript; charset=utf-8",
            )
        } else if path == "/data.json" {
            file_response(&snap_path, "application/json; charset=utf-8")
        } else if let Some(name) = path.strip_prefix("/canvas/") {
            canvas_response(&canvas_dir, name)
        } else if let Some(task_id) = parse_task_logs_path(path) {
            task_logs_response(&store, task_id, &raw_path)
        } else {
            (
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found".to_vec(),
            )
        };

        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    }

    stop.store(true, Ordering::Relaxed);
    Ok(())
}

fn file_response(path: &Path, ctype: &'static str) -> (&'static str, &'static str, Vec<u8>) {
    match fs::read(path) {
        Ok(bytes) => ("200 OK", ctype, bytes),
        Err(_) => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            format!("missing {}", path.display()).into_bytes(),
        ),
    }
}

fn canvas_response(canvas_dir: &Path, name: &str) -> (&'static str, &'static str, Vec<u8>) {
    if name.is_empty()
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return (
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"invalid canvas path".to_vec(),
        );
    }
    let path = canvas_dir.join(name);
    let ctype = content_type_for(name);
    match fs::read(&path) {
        Ok(bytes) => ("200 OK", ctype, bytes),
        Err(_) => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"canvas asset not found".to_vec(),
        ),
    }
}

/// Extracts `<id>` from a path matching exactly `/api/v1/tasks/<id>/logs`
/// (no extra segments before or after). Returns `None` for anything else,
/// including the bare `/api/v1/tasks/<id>` (no `/logs` suffix) so unrelated
/// future task endpoints can't accidentally hit the log reader.
fn parse_task_logs_path(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/api/v1/tasks/")?;
    let id = rest.strip_suffix("/logs")?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id)
}

/// Strict task-id validation: porq ids are UUIDv4 (`new_id()` in
/// `orq-core::types`), but this stays a little more permissive than a UUID
/// regex (alnum/`-`/`_`, bounded length) so it doesn't silently break if the
/// id scheme ever changes, while still rejecting anything that could be a
/// path-traversal or injection attempt (`.`, `/`, `\`, whitespace, etc.).
fn is_valid_task_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn parse_tail_bytes_query(raw_path: &str) -> u64 {
    let query = match raw_path.split_once('?') {
        Some((_, q)) => q,
        None => return DEFAULT_LOG_TAIL_BYTES,
    };
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("tailBytes=") {
            if let Ok(n) = value.parse::<u64>() {
                return n.clamp(1, MAX_LOG_TAIL_BYTES);
            }
        }
    }
    DEFAULT_LOG_TAIL_BYTES
}

/// Read-only, best-effort log tail for the task detail drawer. Never
/// mutates anything; a missing task/log/file is a plain 404, not an error
/// that would need mutation-style handling.
fn task_logs_response(
    store: &Store,
    task_id: &str,
    raw_path: &str,
) -> (&'static str, &'static str, Vec<u8>) {
    let json_error = |status: &'static str, msg: &str| {
        (
            status,
            "application/json; charset=utf-8",
            serde_json::json!({ "ok": false, "error": msg }).to_string().into_bytes(),
        )
    };

    if !is_valid_task_id(task_id) {
        return json_error("400 Bad Request", "invalid task id");
    }
    let task = match store.get_task(task_id) {
        Ok(Some(t)) => t,
        Ok(None) => return json_error("404 Not Found", "task not found"),
        Err(e) => return json_error("500 Internal Server Error", &e.to_string()),
    };
    let Some(log_path) = task.log_path.as_deref() else {
        return json_error("404 Not Found", "task has no log_path");
    };
    let tail_bytes = parse_tail_bytes_query(raw_path);
    match read_tail(Path::new(log_path), tail_bytes) {
        Ok((content, size, truncated)) => {
            let body = serde_json::json!({
                "ok": true,
                "task_id": task_id,
                "log_path": log_path,
                "size_bytes": size,
                "tail_bytes": tail_bytes,
                "truncated": truncated,
                "content": String::from_utf8_lossy(&content),
            });
            (
                "200 OK",
                "application/json; charset=utf-8",
                body.to_string().into_bytes(),
            )
        }
        Err(e) => json_error("404 Not Found", &format!("log unreadable: {e}")),
    }
}

/// Reads at most the last `tail_bytes` of `path`. Returns (content, total
/// file size, whether the content was truncated from the file start).
fn read_tail(path: &Path, tail_bytes: u64) -> std::io::Result<(Vec<u8>, u64, bool)> {
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    let truncated = size > tail_bytes;
    let start = size.saturating_sub(tail_bytes);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity((size - start) as usize);
    file.read_to_end(&mut buf)?;
    Ok((buf, size, truncated))
}

fn content_type_for(name: &str) -> &'static str {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}
