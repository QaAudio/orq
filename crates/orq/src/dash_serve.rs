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

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).with_context(|| format!("bind {addr}"))?;
    eprintln!("orq dash serve http://{addr}/ (root={}, data={})", root.display(), snap_path.display());

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

        let (status, ctype, body) = match path {
            "/" | "/index.html" => file_response(&root.join("index.html"), "text/html; charset=utf-8"),
            "/app.js" => file_response(&root.join("app.js"), "application/javascript; charset=utf-8"),
            "/data.json" => file_response(&snap_path, "application/json; charset=utf-8"),
            _ => (
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found".to_vec(),
            ),
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
