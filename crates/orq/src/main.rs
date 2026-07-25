mod dash_serve;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use orq_core::dash::{default_snapshot_path, write_snapshot};
use orq_core::*;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "porq", about = "Progressive multi-agent orchestration CLI", version)]
struct Cli {
    /// Workspace name (lazy-created)
    #[arg(long, short = 'w', global = true, default_value = "default", env = "ORQ_WORKSPACE")]
    workspace: String,

    /// Ephemeral session scope
    #[arg(long, short = 's', global = true, env = "ORQ_SESSION")]
    session: Option<String>,

    /// JSON output
    #[arg(long, global = true)]
    json: bool,

    /// Limit rows for list commands
    #[arg(long, global = true, default_value_t = 50)]
    limit: usize,

    /// Comma-separated fields filter for JSON objects (best-effort)
    #[arg(long, global = true)]
    fields: Option<String>,

    /// Override data directory
    #[arg(long, global = true, env = "ORQ_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Use the remote libSQL/Turso backend (loads ORQ_DB_URL/ORQ_DB_TOKEN
    /// from environment or a .env file found from the current directory up)
    #[arg(long, global = true)]
    remote: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Explicit workspace init / list
    Init {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    Workspace {
        #[command(subcommand)]
        cmd: WorkspaceCmd,
    },
    /// Append-only event log
    Events {
        #[arg(long)]
        after: Option<i64>,
        #[arg(long)]
        follow: bool,
    },
    /// Point of interest commands
    Poi {
        #[command(subcommand)]
        cmd: PoiCmd,
    },
    /// Dashboard canvases (markdown / image / url / html display protocol)
    Canvas {
        #[command(subcommand)]
        cmd: CanvasCmd,
    },
    /// Submit / manage tasks
    Run {
        /// Command to execute
        #[arg(required = true, last = true)]
        command: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_enum, default_value = "oneshot")]
        kind: KindArg,
        #[arg(long, default_value = "shell")]
        profile: String,
        #[arg(long)]
        claim: Vec<String>,
        #[arg(long)]
        after: Vec<String>,
        #[arg(long)]
        needs_poi: Vec<String>,
        #[arg(long, value_enum, default_value = "never")]
        restart: RestartArg,
        #[arg(long, default_value_t = 1)]
        max_attempts: u32,
        /// Run synchronously without daemon
        #[arg(long)]
        sync: bool,
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Task class override (enables job routing when set with strategy/model)
        #[arg(long)]
        class: Option<String>,
        #[arg(long, value_enum)]
        strategy: Option<StrategyArg>,
        #[arg(long, value_enum, default_value = "sticky")]
        policy: PolicyArg,
        #[arg(long, default_value_t = 3)]
        moa_k: u32,
        #[arg(long, default_value_t = 1)]
        moa_layers: u32,
        #[arg(long)]
        moa_aggregator: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        capability: Option<String>,
        /// Force a specific model id (creates a routed job)
        #[arg(long)]
        model: Option<String>,
    },
    Model {
        #[command(subcommand)]
        cmd: ModelCmd,
    },
    Affinity {
        #[command(subcommand)]
        cmd: AffinityCmd,
    },
    Eval {
        #[command(subcommand)]
        cmd: EvalCmd,
    },
    Job {
        #[command(subcommand)]
        cmd: JobCmd,
    },
    Status,
    Await {
        id: String,
        #[arg(long)]
        timeout_ms: Option<u64>,
    },
    Cancel {
        id: String,
    },
    Kill {
        id: String,
    },
    Interrupt {
        id: String,
    },
    Retry {
        id: String,
    },
    Log {
        id: String,
        #[arg(long)]
        follow: bool,
    },
    Trigger {
        #[command(subcommand)]
        cmd: TriggerCmd,
    },
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },
    Gc,
    Snapshot,
    Report {
        #[arg(long)]
        md: bool,
        #[arg(long)]
        html: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Watch,
    Integrate {
        target: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Live HTTP dashboard (static UI + data.json snapshot)
    Dash {
        #[command(subcommand)]
        cmd: DashCmd,
    },
}

#[derive(Subcommand, Debug)]
enum DashCmd {
    /// Write a JSON snapshot for the dashboard
    Snapshot {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Serve dashboard over localhost HTTP and refresh snapshot every 1s
    Serve {
        #[arg(long, default_value_t = 9847)]
        port: u16,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum CanvasCmd {
    /// Publish or update a canvas POI (exactly one of --md/--body/--image/--url/--html)
    Set {
        key: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value = "live")]
        state: String,
        #[arg(long)]
        order: Option<i64>,
        #[arg(long, default_value_t = 1)]
        span: u8,
        /// Markdown body from a file
        #[arg(long)]
        md: Option<PathBuf>,
        /// Markdown body inline
        #[arg(long)]
        body: Option<String>,
        /// Image file (copied into $ORQ_DATA_DIR/canvas/)
        #[arg(long)]
        image: Option<PathBuf>,
        /// External URL (iframe)
        #[arg(long)]
        url: Option<String>,
        /// HTML/SVG body from a file (sandboxed srcdoc)
        #[arg(long)]
        html: Option<PathBuf>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        alt: Option<String>,
    },
    Ls,
    Rm {
        key: String,
    },
}

#[derive(Subcommand, Debug)]
enum WorkspaceCmd {
    List,
    Budget {
        #[arg(long)]
        max_concurrent: Option<u32>,
        #[arg(long)]
        max_spawns_per_hour: Option<u32>,
    },
    /// Delete every row of a workspace from the store (requires --yes)
    Drop {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PoiCmd {
    Table {
        #[command(subcommand)]
        cmd: PoiTableCmd,
    },
    Ls {
        table: String,
    },
    Get {
        table: String,
        key: String,
    },
    Set {
        table: String,
        key: String,
        value: String,
        #[arg(long)]
        state: Option<String>,
        #[arg(long, value_enum, default_value = "durable")]
        tier: TierArg,
        #[arg(long)]
        if_version: Option<i64>,
        #[arg(long)]
        col: Vec<String>,
    },
    Lock {
        table: String,
        key: String,
        #[arg(long, value_enum, default_value = "write")]
        kind: LockArg,
        #[arg(long, default_value = "agent")]
        holder: String,
        #[arg(long, default_value = "")]
        reason: String,
        #[arg(long, default_value_t = 300)]
        ttl: i64,
    },
    Unlock {
        table: String,
        key: String,
        #[arg(long, default_value = "agent")]
        holder: String,
    },
    Steal {
        table: String,
        key: String,
        #[arg(long, default_value = "agent")]
        holder: String,
        #[arg(long, default_value = "stolen")]
        reason: String,
        #[arg(long, default_value_t = 300)]
        ttl: i64,
    },
    Block {
        table: String,
        key: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Unblock {
        table: String,
        key: String,
    },
}

#[derive(Subcommand, Debug)]
enum PoiTableCmd {
    Create {
        name: String,
        #[arg(long, default_value = "generic")]
        table_type: String,
        /// columns as name:type[:poi]
        #[arg(long = "cols")]
        cols: Vec<String>,
    },
    List,
}

#[derive(Subcommand, Debug)]
enum TriggerCmd {
    Add {
        name: String,
        #[arg(long)]
        on: String,
        #[arg(long)]
        where_cond: Option<String>,
        /// action JSON array or shorthand spawn:CMD / cancel:SEL / set:table/key=VALUE
        #[arg(long)]
        do_action: Vec<String>,
        #[arg(long)]
        blocking: bool,
        #[arg(long, default_value_t = 60)]
        max_fires_per_hour: u32,
    },
    List,
    Enable { id: String },
    Disable { id: String },
}

#[derive(Subcommand, Debug)]
enum DaemonCmd {
    Run,
    Stop,
    Status,
}

#[derive(Subcommand, Debug)]
enum ModelCmd {
    Add {
        id: String,
        #[arg(long)]
        cli: String,
        #[arg(long = "capability", default_value = "generic")]
        capability: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        cost: f64,
        #[arg(long, default_value_t = 1.0)]
        latency: f64,
        #[arg(long)]
        http_stub: bool,
    },
    List,
    Get { id: String },
}

#[derive(Subcommand, Debug)]
enum AffinityCmd {
    Ls {
        #[arg(long)]
        class: Option<String>,
    },
    Set {
        class: String,
        model: String,
        #[arg(long)]
        score: f64,
    },
    Epoch {
        #[command(subcommand)]
        cmd: EpochCmd,
    },
}

#[derive(Subcommand, Debug)]
enum EpochCmd {
    Show,
    Bump,
}

#[derive(Subcommand, Debug)]
enum EvalCmd {
    Show {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        class: Option<String>,
        #[arg(long)]
        claim: Vec<String>,
        #[arg(required = true, last = true)]
        command: Vec<String>,
    },
    SetHook {
        command: String,
    },
}

#[derive(Subcommand, Debug)]
enum JobCmd {
    Status { id: String },
    List,
    Report { id: String },
    Await {
        id: String,
        #[arg(long)]
        sync: bool,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum StrategyArg {
    Single,
    Race,
    Moa,
}

#[derive(Clone, Debug, ValueEnum)]
enum PolicyArg {
    Sticky,
    Epsilon,
    Softmax,
}

#[derive(Clone, Debug, ValueEnum)]
enum TierArg {
    Ephemeral,
    Durable,
    Versioned,
}

#[derive(Clone, Debug, ValueEnum)]
enum LockArg {
    Write,
    ReadBlock,
}

#[derive(Clone, Debug, ValueEnum)]
enum KindArg {
    Oneshot,
    Service,
}

#[derive(Clone, Debug, ValueEnum)]
enum RestartArg {
    Never,
    OnFailure,
    Always,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.remote {
        // Opt-in only: otherwise a stray .env would silently point local
        // runs (and tests) at the cloud database.
        let _ = dotenvy::dotenv();
        if std::env::var("ORQ_DB_URL").map(|v| v.trim().is_empty()).unwrap_or(true) {
            bail!("--remote requires ORQ_DB_URL (and usually ORQ_DB_TOKEN) in the environment or a .env file");
        }
    }
    let data_dir = match &cli.data_dir {
        Some(p) => p.clone(),
        None => default_data_dir().context("data dir")?,
    };
    std::env::set_var("ORQ_DATA_DIR", &data_dir);
    let store = Store::open_env(&data_dir)?;

    let early_opts = OutOpts {
        json: cli.json,
        fields: cli.fields.clone(),
    };
    match &cli.cmd {
        Commands::Daemon { cmd: DaemonCmd::Run } => {
            run_daemon(store)?;
            return Ok(());
        }
        Commands::Daemon { cmd: DaemonCmd::Stop } => {
            if is_daemon_running(&data_dir) {
                let resp = send_request(&data_dir, &DaemonRequest::Shutdown)?;
                out(&early_opts, &resp)?;
            } else {
                out(
                    &early_opts,
                    &json!({"ok": true, "message": "daemon not running"}),
                )?;
            }
            return Ok(());
        }
        Commands::Daemon {
            cmd: DaemonCmd::Status,
        } => {
            if is_daemon_running(&data_dir) {
                let resp = send_request(&data_dir, &DaemonRequest::Status)?;
                out(&early_opts, &resp)?;
            } else {
                out(
                    &early_opts,
                    &json!({"ok": false, "message": "daemon not running"}),
                )?;
            }
            return Ok(());
        }
        Commands::Watch => {
            orq_tui::run_watch(&store, &cli.workspace)?;
            return Ok(());
        }
        Commands::Dash {
            cmd: DashCmd::Serve { port, root, out },
        } => {
            store.ensure_workspace(&cli.workspace, None)?;
            let root = dash_serve::resolve_dash_root(root.clone());
            dash_serve::run_serve(
                Arc::new(store),
                cli.workspace.clone(),
                cli.session.clone(),
                data_dir.clone(),
                *port,
                root,
                out.clone(),
            )?;
            return Ok(());
        }
        _ => {}
    }

    // Lazy workspace
    store.ensure_workspace(&cli.workspace, None)?;
    let session = cli.session.clone();
    let workspace = cli.workspace.clone();
    let json_out = cli.json;
    let limit = cli.limit;
    let fields = cli.fields.clone();
    let out_opts = OutOpts {
        json: json_out,
        fields,
    };
    let session = session.as_deref();
    let cmd = cli.cmd;

    match cmd {
        Commands::Init { root } => {
            let ws = store.ensure_workspace(
                &workspace,
                root.as_ref().map(|p| p.to_str().unwrap_or(".")),
            )?;
            out(&out_opts, &ws)?;
        }
        Commands::Workspace {
            cmd: WorkspaceCmd::List,
        } => {
            let list = store.list_workspaces()?;
            out(&out_opts, &list)?;
        }
        Commands::Workspace {
            cmd: WorkspaceCmd::Budget {
                max_concurrent,
                max_spawns_per_hour,
            },
        } => {
            store.set_workspace_budgets(&workspace, max_concurrent, max_spawns_per_hour)?;
            out(&out_opts, &store.get_workspace(&workspace)?)?;
        }
        Commands::Workspace {
            cmd: WorkspaceCmd::Drop { name, yes },
        } => {
            if !yes {
                bail!("workspace drop is destructive; re-run with --yes");
            }
            store.drop_workspace(&name)?;
            out(&out_opts, &json!({ "dropped": name }))?;
        }
        Commands::Events { after, follow } => {
            if follow {
                let mut last = after.unwrap_or(0);
                loop {
                    let evs = store.list_events(&workspace, Some(last), limit, session)?;
                    for e in &evs {
                        out(&out_opts, e)?;
                        last = e.id;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            } else {
                let evs = store.list_events(&workspace, after, limit, session)?;
                out(&out_opts, &evs)?;
            }
        }
        Commands::Poi { cmd } => handle_poi(&out_opts, &workspace, session, limit, &store, cmd)?,
        Commands::Canvas { cmd } => {
            handle_canvas(&out_opts, &workspace, session, limit, &store, &data_dir, cmd)?
        }
        Commands::Run {
            command,
            name,
            kind,
            profile,
            claim,
            after,
            needs_poi,
            restart,
            max_attempts,
            sync,
            cwd,
            class,
            strategy,
            policy,
            moa_k,
            moa_layers,
            moa_aggregator,
            seed,
            capability,
            model,
        } => {
            let cmd = command.join(" ");
            let name = name.unwrap_or_else(|| "task".into());
            let use_job = strategy.is_some() || class.is_some() || model.is_some();
            if use_job {
                let strat = match strategy.unwrap_or(StrategyArg::Single) {
                    StrategyArg::Single => ScheduleStrategy::Single,
                    StrategyArg::Race => ScheduleStrategy::Race,
                    StrategyArg::Moa => ScheduleStrategy::Moa,
                };
                let pol = match policy {
                    PolicyArg::Sticky => RoutePolicy::Sticky,
                    PolicyArg::Epsilon => RoutePolicy::Epsilon,
                    PolicyArg::Softmax => RoutePolicy::Softmax,
                };
                let job_engine = JobEngine::new(&store);
                let job = job_engine.submit(JobSubmitOpts {
                    workspace: &workspace,
                    session,
                    name: &name,
                    command: &cmd,
                    claims: claim,
                    class_override: class.as_deref(),
                    strategy: strat,
                    policy: pol,
                    k: moa_k,
                    moa_layers,
                    aggregator: moa_aggregator.as_deref(),
                    seed,
                    capability: capability.as_deref(),
                    force_model: model.as_deref(),
                    cwd: cwd.as_ref().and_then(|p| p.to_str()),
                })?;
                if sync {
                    let done = job_engine.run_sync(&job.id)?;
                    out(&out_opts, &done)?;
                } else {
                    let _ = ensure_daemon(&data_dir);
                    out(&out_opts, &job)?;
                }
            } else {
                let kind = match kind {
                    KindArg::Oneshot => TaskKind::Oneshot,
                    KindArg::Service => TaskKind::Service,
                };
                let restart = match restart {
                    RestartArg::Never => RestartPolicy::Never,
                    RestartArg::OnFailure => RestartPolicy::OnFailure,
                    RestartArg::Always => RestartPolicy::Always,
                };
                let engine = TaskEngine::new(&store);
                let task = engine.submit(
                    &workspace,
                    session,
                    &name,
                    &cmd,
                    kind,
                    &profile,
                    claim,
                    after,
                    needs_poi,
                    restart,
                    max_attempts,
                    cwd.as_ref().and_then(|p| p.to_str()),
                )?;
                if sync {
                    let done = engine.run_to_completion(&task.id)?;
                    out(&out_opts, &done)?;
                } else {
                    let _ = ensure_daemon(&data_dir);
                    out(&out_opts, &task)?;
                }
            }
        }
        Commands::Model { cmd } => handle_model(&out_opts, &workspace, &store, cmd)?,
        Commands::Affinity { cmd } => handle_affinity(&out_opts, &workspace, &store, cmd)?,
        Commands::Eval { cmd } => handle_eval(&out_opts, &workspace, &store, cmd)?,
        Commands::Job { cmd } => handle_job(&out_opts, &workspace, &store, &data_dir, cmd)?,
        Commands::Status => {
            let v = status_json(&store, &workspace, session, limit)?;
            if out_opts.json {
                out(&out_opts, &v)?;
            } else {
                println!("workspace: {workspace}");
                if let Some(tasks) = v.get("tasks").and_then(|t| t.as_array()) {
                    println!(
                        "{:<10} {:<16} {:<12} {:<10} {}",
                        "ID", "NAME", "STATUS", "KIND", "CMD"
                    );
                    for t in tasks.iter().take(limit) {
                        println!(
                            "{:<10} {:<16} {:<12} {:<10} {}",
                            short(t.get("id").and_then(|x| x.as_str()).unwrap_or("")),
                            trunc(t.get("name").and_then(|x| x.as_str()).unwrap_or(""), 16),
                            t.get("status").and_then(|x| x.as_str()).unwrap_or(""),
                            t.get("kind").and_then(|x| x.as_str()).unwrap_or(""),
                            trunc(t.get("command").and_then(|x| x.as_str()).unwrap_or(""), 40),
                        );
                    }
                }
            }
        }
        Commands::Await { id, timeout_ms } => {
            let engine = TaskEngine::new(&store);
            if !is_daemon_running(&data_dir) {
                let _ = engine.run_to_completion(&id);
            }
            let t = engine.await_task(&id, timeout_ms)?;
            out(&out_opts, &t)?;
        }
        Commands::Cancel { id } => {
            let t = TaskEngine::new(&store).cancel(&id, false)?;
            out(&out_opts, &t)?;
        }
        Commands::Kill { id } => {
            let t = TaskEngine::new(&store).cancel(&id, true)?;
            out(&out_opts, &t)?;
        }
        Commands::Interrupt { id } => {
            let t = TaskEngine::new(&store).interrupt(&id)?;
            out(&out_opts, &t)?;
        }
        Commands::Retry { id } => {
            let old = store
                .get_task(&id)?
                .ok_or_else(|| anyhow::anyhow!("task not found"))?;
            let engine = TaskEngine::new(&store);
            let t = engine.submit(
                &old.workspace,
                old.session.as_deref(),
                &format!("{}-retry", old.name),
                &old.command,
                old.kind,
                &old.profile,
                old.claims.clone(),
                vec![],
                old.needs_poi.clone(),
                old.restart,
                old.max_attempts,
                old.cwd.as_deref(),
            )?;
            out(&out_opts, &t)?;
        }
        Commands::Log { id, follow } => {
            let task = store
                .get_task(&id)?
                .ok_or_else(|| anyhow::anyhow!("task not found"))?;
            let path = task
                .log_path
                .ok_or_else(|| anyhow::anyhow!("no log path"))?;
            if follow {
                let mut pos = 0u64;
                loop {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if meta.len() > pos {
                            let data = std::fs::read(&path)?;
                            if (pos as usize) < data.len() {
                                print!("{}", String::from_utf8_lossy(&data[pos as usize..]));
                                pos = data.len() as u64;
                            }
                        }
                    }
                    let fresh = store.get_task(&id)?;
                    if fresh.as_ref().map(|t| t.status.is_terminal()).unwrap_or(true) {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if let Ok(meta) = std::fs::metadata(&path) {
                            if meta.len() == pos {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            } else if out_opts.json {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                out(&out_opts, &json!({"path": path, "content": content}))?;
            } else {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                print!("{content}");
            }
        }
        Commands::Trigger { cmd } => {
            handle_trigger(&out_opts, &workspace, &store, cmd)?
        }
        Commands::Gc => {
            let r = store.gc(&workspace, session)?;
            out(&out_opts, &r)?;
        }
        Commands::Snapshot => {
            let p = store.export_workspace_snapshot(&workspace)?;
            out(&out_opts, &json!({"path": p.display().to_string()}))?;
        }
        Commands::Report { md, html, out: out_path } => {
            let content = if html {
                render_html(&store, &workspace, limit)?
            } else {
                let _ = md;
                render_markdown(&store, &workspace, limit)?
            };
            if let Some(p) = out_path {
                std::fs::write(&p, &content)?;
                out(&out_opts, &json!({"written": p.display().to_string()}))?;
            } else if out_opts.json {
                out(&out_opts, &json!({"report": content}))?;
            } else {
                print!("{content}");
            }
        }
        Commands::Integrate { target, path } => {
            if target != "cursor" {
                bail!("unsupported integrate target: {target} (supported: cursor)");
            }
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            let written = integrate_cursor(&root)?;
            out(&out_opts, &json!({"written": written}))?;
        }
        Commands::Dash {
            cmd: DashCmd::Snapshot { out: out_path },
        } => {
            let path = out_path.unwrap_or_else(|| default_snapshot_path(&data_dir));
            let snap = write_snapshot(&store, &workspace, session, &path)?;
            out(
                &out_opts,
                &json!({
                    "ok": true,
                    "path": path,
                    "updated": snap.get("updated"),
                    "board": snap.get("board").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                    "tasks": snap.get("tasks").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                }),
            )?;
        }
        Commands::Daemon { .. } | Commands::Watch | Commands::Dash { .. } => unreachable!(),
    }
    Ok(())
}

#[derive(Clone)]
struct OutOpts {
    json: bool,
    fields: Option<String>,
}

fn ensure_canvas_table(store: &Store, workspace: &str) -> Result<()> {
    if store.get_poi_table(workspace, "canvas")?.is_none() {
        store.create_poi_table(
            workspace,
            "canvas",
            "canvas",
            vec![
                ColumnDef {
                    name: "order".into(),
                    col_type: "number".into(),
                    poi: false,
                },
                ColumnDef {
                    name: "span".into(),
                    col_type: "number".into(),
                    poi: false,
                },
            ],
        )?;
    }
    Ok(())
}

fn sanitize_canvas_filename(name: &str) -> Result<String> {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("asset");
    if base.is_empty()
        || base.contains("..")
        || base.contains('/')
        || base.contains('\\')
        || base.contains('\0')
    {
        bail!("invalid canvas asset filename: {name}");
    }
    Ok(base.to_string())
}

fn handle_canvas(
    opts: &OutOpts,
    workspace: &str,
    session: Option<&str>,
    limit: usize,
    store: &Store,
    data_dir: &Path,
    cmd: CanvasCmd,
) -> Result<()> {
    match cmd {
        CanvasCmd::Set {
            key,
            title,
            state,
            order,
            span,
            md,
            body,
            image,
            url,
            html,
            height,
            alt,
        } => {
            let sources = [
                md.is_some(),
                body.is_some(),
                image.is_some(),
                url.is_some(),
                html.is_some(),
            ]
            .into_iter()
            .filter(|b| *b)
            .count();
            if sources != 1 {
                bail!("canvas set requires exactly one of --md, --body, --image, --url, --html");
            }
            let span = if span == 2 { 2 } else { 1 };
            ensure_canvas_table(store, workspace)?;

            let value = if let Some(path) = md {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("read markdown {}", path.display()))?;
                json!({
                    "v": 1,
                    "kind": "markdown",
                    "title": title.unwrap_or_else(|| key.clone()),
                    "body": text,
                })
            } else if let Some(text) = body {
                json!({
                    "v": 1,
                    "kind": "markdown",
                    "title": title.unwrap_or_else(|| key.clone()),
                    "body": text,
                })
            } else if let Some(path) = image {
                let fname = sanitize_canvas_filename(
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("image.png"),
                )?;
                let dest_dir = data_dir.join("canvas");
                std::fs::create_dir_all(&dest_dir)?;
                let dest = dest_dir.join(&fname);
                std::fs::copy(&path, &dest)
                    .with_context(|| format!("copy image {} -> {}", path.display(), dest.display()))?;
                let mut obj = json!({
                    "v": 1,
                    "kind": "image",
                    "title": title.unwrap_or_else(|| key.clone()),
                    "src": format!("canvas:{fname}"),
                });
                if let Some(a) = alt {
                    obj["alt"] = json!(a);
                }
                obj
            } else if let Some(src) = url {
                let mut obj = json!({
                    "v": 1,
                    "kind": "url",
                    "title": title.unwrap_or_else(|| key.clone()),
                    "src": src,
                });
                if let Some(h) = height {
                    obj["height"] = json!(h);
                }
                obj
            } else if let Some(path) = html {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("read html {}", path.display()))?;
                let mut obj = json!({
                    "v": 1,
                    "kind": "html",
                    "title": title.unwrap_or_else(|| key.clone()),
                    "body": text,
                });
                if let Some(h) = height {
                    obj["height"] = json!(h);
                }
                obj
            } else {
                unreachable!()
            };

            let mut columns = HashMap::new();
            columns.insert("span".into(), json!(span));
            if let Some(o) = order {
                columns.insert("order".into(), json!(o));
            }

            let poi = store.set_poi(
                workspace,
                "canvas",
                &key,
                value,
                columns,
                Some(&state),
                StorageTier::Durable,
                session,
                None,
                None,
            )?;
            out(opts, &poi)?;
        }
        CanvasCmd::Ls => {
            ensure_canvas_table(store, workspace)?;
            let rows = store.list_pois(workspace, "canvas", session, limit)?;
            out(opts, &rows)?;
        }
        CanvasCmd::Rm { key } => {
            let deleted = store.delete_poi(workspace, "canvas", &key)?;
            out(opts, &json!({ "deleted": deleted, "key": key }))?;
        }
    }
    Ok(())
}

fn handle_poi(
    opts: &OutOpts,
    workspace: &str,
    session: Option<&str>,
    limit: usize,
    store: &Store,
    cmd: PoiCmd,
) -> Result<()> {
    match cmd {
        PoiCmd::Table {
            cmd: PoiTableCmd::Create {
                name,
                table_type,
                cols,
            },
        } => {
            let columns = parse_cols(&cols);
            let t = store.create_poi_table(workspace, &name, &table_type, columns)?;
            out(opts, &t)?;
        }
        PoiCmd::Table {
            cmd: PoiTableCmd::List,
        } => {
            out(opts, &store.list_poi_tables(workspace)?)?;
        }
        PoiCmd::Ls { table } => {
            let rows = store.list_pois(workspace, &table, session, limit)?;
            if opts.json {
                out(opts, &rows)?;
            } else {
                println!(
                    "{:<24} {:<8} {:<12} {:<8} {}",
                    "KEY", "VER", "STATE", "BLOCK", "VALUE"
                );
                for p in rows {
                    println!(
                        "{:<24} {:<8} {:<12} {:<8} {}",
                        trunc(&p.key, 24),
                        p.version,
                        trunc(&p.state, 12),
                        p.blocked,
                        trunc(&p.value.to_string(), 40)
                    );
                }
            }
        }
        PoiCmd::Get { table, key } => {
            let p = store
                .get_poi(workspace, &table, &key)?
                .ok_or_else(|| anyhow::anyhow!("poi not found"))?;
            out(opts, &p)?;
        }
        PoiCmd::Set {
            table,
            key,
            value,
            state,
            tier,
            if_version,
            col,
        } => {
            let value: serde_json::Value = serde_json::from_str(&value)
                .unwrap_or(serde_json::Value::String(value.clone()));
            let mut columns = HashMap::new();
            for c in col {
                if let Some((k, v)) = c.split_once('=') {
                    let vv: serde_json::Value =
                        serde_json::from_str(v).unwrap_or(serde_json::Value::String(v.into()));
                    columns.insert(k.to_string(), vv);
                }
            }
            let tier = match tier {
                TierArg::Ephemeral => StorageTier::Ephemeral,
                TierArg::Durable => StorageTier::Durable,
                TierArg::Versioned => StorageTier::Versioned,
            };
            let poi = store.set_poi(
                workspace,
                &table,
                &key,
                value.clone(),
                columns,
                state.as_deref(),
                tier,
                session,
                if_version,
                None,
            )?;
            let engine = TriggerEngine::new(store);
            let mut seen = HashSet::new();
            engine.process_event(
                workspace,
                session,
                "poi.changed",
                &json!({
                    "table": table,
                    "key": key,
                    "version": poi.version,
                    "state": poi.state,
                    "value": value,
                }),
                0,
                &mut seen,
            )?;
            out(opts, &poi)?;
        }
        PoiCmd::Lock {
            table,
            key,
            kind,
            holder,
            reason,
            ttl,
        } => {
            let kind = match kind {
                LockArg::Write => LeaseKind::Write,
                LockArg::ReadBlock => LeaseKind::ReadBlock,
            };
            let lease =
                store.acquire_lease(workspace, &table, &key, kind, &holder, &reason, ttl)?;
            out(opts, &lease)?;
        }
        PoiCmd::Unlock {
            table,
            key,
            holder,
        } => {
            store.release_lease(workspace, &table, &key, &holder)?;
            out(opts, &json!({"ok": true}))?;
        }
        PoiCmd::Steal {
            table,
            key,
            holder,
            reason,
            ttl,
        } => {
            let lease = store.steal_lease(workspace, &table, &key, &holder, &reason, ttl)?;
            out(opts, &lease)?;
        }
        PoiCmd::Block { table, key, reason } => {
            let p = store.set_poi_blocked(workspace, &table, &key, true, reason.as_deref())?;
            out(opts, &p)?;
        }
        PoiCmd::Unblock { table, key } => {
            let p = store.set_poi_blocked(workspace, &table, &key, false, None)?;
            out(opts, &p)?;
        }
    }
    Ok(())
}

fn handle_trigger(
    opts: &OutOpts,
    workspace: &str,
    store: &Store,
    cmd: TriggerCmd,
) -> Result<()> {
    match cmd {
        TriggerCmd::Add {
            name,
            on,
            where_cond,
            do_action,
            blocking,
            max_fires_per_hour,
        } => {
            let mut actions = Vec::new();
            for a in do_action {
                actions.push(parse_action(&a)?);
            }
            let (pattern, cond) = if on.starts_with("poi.state==") {
                ("poi.changed".into(), Some(on.clone()))
            } else {
                (on, where_cond)
            };
            let rule = TriggerEngine::new(store).add(
                workspace,
                &name,
                &pattern,
                cond.as_deref(),
                actions,
                blocking,
                max_fires_per_hour,
            )?;
            out(opts, &rule)?;
        }
        TriggerCmd::List => out(opts, &store.list_triggers(workspace)?)?,
        TriggerCmd::Enable { id } => {
            store.set_trigger_enabled(&id, true)?;
            out(opts, &json!({"ok": true, "id": id, "enabled": true}))?;
        }
        TriggerCmd::Disable { id } => {
            store.set_trigger_enabled(&id, false)?;
            out(opts, &json!({"ok": true, "id": id, "enabled": false}))?;
        }
    }
    Ok(())
}

fn handle_model(opts: &OutOpts, workspace: &str, store: &Store, cmd: ModelCmd) -> Result<()> {
    store.ensure_workspace(workspace, None)?;
    match cmd {
        ModelCmd::Add {
            id,
            cli,
            capability,
            name,
            cost,
            latency,
            http_stub,
        } => {
            let recipe = LaunchRecipe {
                kind: if http_stub {
                    LaunchKind::HttpStub
                } else {
                    LaunchKind::Cli
                },
                template: cli,
                model_arg: Some(id.clone()),
                env: HashMap::new(),
            };
            let m = ModelRegistry::new(store).add(
                workspace,
                &id,
                name.as_deref(),
                capability,
                cost,
                latency,
                recipe,
            )?;
            out(opts, &m)?;
        }
        ModelCmd::List => out(opts, &ModelRegistry::new(store).list(workspace)?)?,
        ModelCmd::Get { id } => out(opts, &ModelRegistry::new(store).get(workspace, &id)?)?,
    }
    Ok(())
}

fn handle_affinity(
    opts: &OutOpts,
    workspace: &str,
    store: &Store,
    cmd: AffinityCmd,
) -> Result<()> {
    store.ensure_workspace(workspace, None)?;
    let eng = AffinityEngine::new(store);
    match cmd {
        AffinityCmd::Ls { class } => {
            if let Some(ref c) = class {
                let _ = eng.ensure_defaults(workspace, c);
            }
            out(opts, &eng.list(workspace, class.as_deref())?)?;
        }
        AffinityCmd::Set {
            class,
            model,
            score,
        } => {
            out(opts, &eng.set(workspace, &class, &model, score)?)?;
        }
        AffinityCmd::Epoch { cmd: EpochCmd::Show } => {
            out(opts, &json!({ "epoch": eng.epoch(workspace)? }))?;
        }
        AffinityCmd::Epoch { cmd: EpochCmd::Bump } => {
            out(opts, &json!({ "epoch": eng.bump_epoch(workspace)? }))?;
        }
    }
    Ok(())
}

fn handle_eval(opts: &OutOpts, workspace: &str, store: &Store, cmd: EvalCmd) -> Result<()> {
    store.ensure_workspace(workspace, None)?;
    match cmd {
        EvalCmd::Show {
            name,
            class,
            claim,
            command,
        } => {
            let cmd_s = command.join(" ");
            let name = name.unwrap_or_else(|| "eval".into());
            let r = Evaluator::new(store).evaluate(
                workspace,
                &name,
                &cmd_s,
                &claim,
                &[],
                class.as_deref(),
            )?;
            out(opts, &r)?;
        }
        EvalCmd::SetHook { command } => {
            store.set_eval_hook(workspace, &command)?;
            out(opts, &json!({ "ok": true, "hook": command }))?;
        }
    }
    Ok(())
}

fn handle_job(
    opts: &OutOpts,
    workspace: &str,
    store: &Store,
    data_dir: &PathBuf,
    cmd: JobCmd,
) -> Result<()> {
    match cmd {
        JobCmd::Status { id } => {
            let j = store
                .get_job(&id)?
                .ok_or_else(|| anyhow::anyhow!("job not found"))?;
            out(opts, &j)?;
        }
        JobCmd::List => out(opts, &store.list_jobs(workspace, 50)?)?,
        JobCmd::Report { id } => {
            let md = JobEngine::new(store).report_md(&id)?;
            if opts.json {
                out(opts, &json!({ "report": md }))?;
            } else {
                print!("{md}");
            }
        }
        JobCmd::Await { id, sync } => {
            let eng = JobEngine::new(store);
            let j = if sync || !is_daemon_running(data_dir) {
                eng.run_sync(&id)?
            } else {
                loop {
                    let j = eng.tick_job(&id)?;
                    if j.status.is_terminal() {
                        break j;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            };
            out(opts, &j)?;
        }
    }
    Ok(())
}

fn parse_action(s: &str) -> Result<TriggerAction> {
    if let Ok(v) = serde_json::from_str::<TriggerAction>(s) {
        return Ok(v);
    }
    if let Some(cmd) = s.strip_prefix("spawn:") {
        return Ok(TriggerAction::SpawnTask {
            command: cmd.into(),
            name: Some("triggered".into()),
            kind: None,
        });
    }
    if let Some(sel) = s.strip_prefix("cancel:") {
        return Ok(TriggerAction::CancelTasks {
            selector: sel.into(),
        });
    }
    if let Some(rest) = s.strip_prefix("set:") {
        // set:table/key=value
        let (path, val) = rest
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("bad set action"))?;
        let (table, key) = path
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("set needs table/key"))?;
        let value: serde_json::Value =
            serde_json::from_str(val).unwrap_or(serde_json::Value::String(val.into()));
        return Ok(TriggerAction::SetPoi {
            table: table.into(),
            key: key.into(),
            value,
            state: None,
        });
    }
    if let Some(cmd) = s.strip_prefix("hook:") {
        return Ok(TriggerAction::RunHook {
            command: cmd.into(),
        });
    }
    bail!("unknown action: {s}");
}

fn parse_cols(cols: &[String]) -> Vec<ColumnDef> {
    if cols.is_empty() {
        return vec![ColumnDef {
            name: "value".into(),
            col_type: "json".into(),
            poi: true,
        }];
    }
    cols.iter()
        .map(|c| {
            let parts: Vec<_> = c.split(':').collect();
            ColumnDef {
                name: parts.first().unwrap_or(&"col").to_string(),
                col_type: parts.get(1).unwrap_or(&"string").to_string(),
                poi: parts.get(2).map(|p| *p == "poi").unwrap_or(false),
            }
        })
        .collect()
}

fn out<T: serde::Serialize>(opts: &OutOpts, val: &T) -> Result<()> {
    let mut v = serde_json::to_value(val)?;
    if let Some(ref fields) = opts.fields {
        v = filter_fields(v, fields);
    }
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn filter_fields(v: serde_json::Value, fields: &str) -> serde_json::Value {
    let want: HashSet<&str> = fields.split(',').map(|s| s.trim()).collect();
    match v {
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.into_iter()
                .map(|item| filter_fields(item, fields))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                if want.contains(k.as_str()) {
                    out.insert(k, val);
                }
            }
            serde_json::Value::Object(out)
        }
        other => other,
    }
}

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}

fn trunc(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n.saturating_sub(1)])
    }
}
