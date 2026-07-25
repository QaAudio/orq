use crate::error::{OrqError, Result};
use crate::store::Store;
use crate::task::{run_daemon_loop, DaemonState};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum DaemonRequest {
    Ping,
    Shutdown,
    Tick { workspace: String },
    Status,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub fn daemon_port_file(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("daemon.port")
}

pub fn daemon_pid_file(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("daemon.pid")
}

pub fn is_daemon_running(data_dir: &std::path::Path) -> bool {
    let port_file = daemon_port_file(data_dir);
    if let Ok(port_s) = std::fs::read_to_string(&port_file) {
        if let Ok(port) = port_s.trim().parse::<u16>() {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return true;
            }
        }
    }
    false
}

pub fn ensure_daemon(data_dir: &std::path::Path) -> Result<()> {
    if is_daemon_running(data_dir) {
        return Ok(());
    }
    // Spawn detached `orq daemon run`
    let exe = std::env::current_exe().map_err(OrqError::from)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["daemon", "run"])
            .env("ORQ_DATA_DIR", data_dir)
            .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
        cmd.spawn().map_err(OrqError::from)?;
    }
    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["daemon", "run"])
            .env("ORQ_DATA_DIR", data_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        cmd.spawn().map_err(OrqError::from)?;
    }
    // Wait briefly for port file
    for _ in 0..50 {
        if is_daemon_running(data_dir) {
            return Ok(());
        }
        thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(OrqError::Daemon("failed to start daemon".into()))
}

pub fn send_request(data_dir: &std::path::Path, req: &DaemonRequest) -> Result<DaemonResponse> {
    let port_s = std::fs::read_to_string(daemon_port_file(data_dir))
        .map_err(|_| OrqError::Daemon("daemon not running".into()))?;
    let port: u16 = port_s
        .trim()
        .parse()
        .map_err(|_| OrqError::Daemon("bad port file".into()))?;
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| OrqError::Daemon(format!("connect: {e}")))?;
    let line = serde_json::to_string(req)? + "\n";
    stream.write_all(line.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    reader.read_line(&mut resp)?;
    Ok(serde_json::from_str(&resp)?)
}

pub fn run_daemon(store: Store) -> Result<()> {
    let data_dir = store.data_dir.clone();
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    std::fs::write(daemon_port_file(&data_dir), port.to_string())?;
    std::fs::write(daemon_pid_file(&data_dir), std::process::id().to_string())?;

    let store = Arc::new(store);
    let state = Arc::new(DaemonState::new());
    let stop = Arc::new(Mutex::new(false));
    let stop_bg = Arc::clone(&stop);
    let store_bg = Arc::clone(&store);
    let state_bg = Arc::clone(&state);
    thread::spawn(move || run_daemon_loop(store_bg, state_bg, stop_bg));

    for conn in listener.incoming() {
        let Ok(mut stream) = conn else { continue };
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            continue;
        }
        let req: DaemonRequest = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                let resp = DaemonResponse {
                    ok: false,
                    message: e.to_string(),
                    data: None,
                };
                let _ = writeln!(stream, "{}", serde_json::to_string(&resp).unwrap_or_default());
                continue;
            }
        };
        let resp = match req {
            DaemonRequest::Ping => DaemonResponse {
                ok: true,
                message: "pong".into(),
                data: None,
            },
            DaemonRequest::Status => {
                let n = state.children.lock().unwrap().len();
                DaemonResponse {
                    ok: true,
                    message: "ok".into(),
                    data: Some(serde_json::json!({ "children": n, "port": port })),
                }
            }
            DaemonRequest::Tick { workspace } => {
                let engine = crate::task::TaskEngine::new(&store);
                match engine.tick_once(&workspace) {
                    Ok(n) => DaemonResponse {
                        ok: true,
                        message: format!("started {n}"),
                        data: Some(serde_json::json!({ "started": n })),
                    },
                    Err(e) => DaemonResponse {
                        ok: false,
                        message: e.to_string(),
                        data: None,
                    },
                }
            }
            DaemonRequest::Shutdown => {
                *stop.lock().unwrap() = true;
                let _ = std::fs::remove_file(daemon_port_file(&data_dir));
                let _ = std::fs::remove_file(daemon_pid_file(&data_dir));
                let resp = DaemonResponse {
                    ok: true,
                    message: "shutting down".into(),
                    data: None,
                };
                let _ = writeln!(stream, "{}", serde_json::to_string(&resp).unwrap_or_default());
                break;
            }
        };
        let _ = writeln!(stream, "{}", serde_json::to_string(&resp).unwrap_or_default());
    }
    Ok(())
}
