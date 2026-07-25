use anyhow::{bail, Context, Result};
use orq_core::dash::{default_snapshot_path, write_snapshot};
use orq_core::Store;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
        let path = req
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        let path = path.split('?').next().unwrap_or("/");

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
