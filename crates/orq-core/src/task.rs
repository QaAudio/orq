use crate::affinity::AffinityEngine;
use crate::error::{OrqError, Result};
use crate::job::JobEngine;
use crate::paths::claims_overlap;
use crate::store::Store;
use crate::trigger::TriggerEngine;
use crate::types::*;
use serde_json::json;
use std::collections::HashSet;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct TaskEngine<'a> {
    store: &'a Store,
}

impl<'a> TaskEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn submit(
        &self,
        workspace: &str,
        session: Option<&str>,
        name: &str,
        command: &str,
        kind: TaskKind,
        profile: &str,
        claims: Vec<String>,
        depends_on: Vec<String>,
        needs_poi: Vec<String>,
        restart: RestartPolicy,
        max_attempts: u32,
        cwd: Option<&str>,
    ) -> Result<Task> {
        let ws = self.store.ensure_workspace(workspace, None)?;
        let spawns = self.store.spawns_last_hour(workspace)?;
        if spawns >= ws.max_spawns_per_hour {
            return Err(OrqError::BudgetExceeded(format!(
                "workspace spawn budget {}/h",
                ws.max_spawns_per_hour
            )));
        }

        // Check claim overlap with active tasks
        let active = self.store.list_active_tasks(workspace)?;
        for other in &active {
            if !other.claims.is_empty()
                && !claims.is_empty()
                && claims_overlap(&claims, &other.claims)
            {
                // Still queue — overlap means we stay queued until free
            }
        }

        let id = new_id();
        let log_path = self.store.log_path_for(&id);

        let task = Task {
            id: id.clone(),
            workspace: workspace.into(),
            session: session.map(|s| s.to_string()),
            name: name.into(),
            kind,
            status: TaskStatus::Queued,
            command: command.into(),
            cwd: cwd.map(|s| s.to_string()).or(Some(ws.root.clone())),
            profile: profile.into(),
            claims,
            depends_on,
            needs_poi,
            restart,
            attempt: 0,
            max_attempts: max_attempts.max(1),
            pid: None,
            exit_code: None,
            log_path: Some(log_path.display().to_string()),
            job_id: None,
            model_id: None,
            class: None,
            role: None,
            created_at: now(),
            updated_at: now(),
        };
        self.store.insert_task(&task)?;
        self.store.record_spawn(workspace)?;
        Ok(task)
    }

    pub fn try_start(&self, task_id: &str) -> Result<Option<Task>> {
        let mut task = self
            .store
            .get_task(task_id)?
            .ok_or_else(|| OrqError::TaskNotFound(task_id.into()))?;
        if task.status != TaskStatus::Queued && task.status != TaskStatus::Blocked {
            return Ok(Some(task));
        }

        // Dependencies
        for dep in &task.depends_on {
            let d = self
                .store
                .get_task(dep)?
                .ok_or_else(|| OrqError::TaskNotFound(dep.clone()))?;
            if !d.status.is_terminal() {
                task.status = TaskStatus::Blocked;
                task.updated_at = now();
                self.store.update_task(&task)?;
                return Ok(Some(task));
            }
            if d.status != TaskStatus::Done {
                task.status = TaskStatus::Failed;
                task.exit_code = Some(1);
                task.updated_at = now();
                self.store.update_task(&task)?;
                return Ok(Some(task));
            }
        }

        // needs_poi: table/key must exist and not be blocked
        for np in &task.needs_poi {
            let (table, key) = split_poi_ref(np);
            let poi = self.store.get_poi(&task.workspace, table, key)?;
            match poi {
                None => {
                    task.status = TaskStatus::Blocked;
                    task.updated_at = now();
                    self.store.update_task(&task)?;
                    return Ok(Some(task));
                }
                Some(p) if p.blocked => {
                    task.status = TaskStatus::Blocked;
                    task.updated_at = now();
                    self.store.update_task(&task)?;
                    return Ok(Some(task));
                }
                _ => {}
            }
        }

        // Claims: check overlap with running tasks
        let active = self.store.list_active_tasks(&task.workspace)?;
        for other in &active {
            if other.id == task.id {
                continue;
            }
            if matches!(
                other.status,
                TaskStatus::Starting | TaskStatus::Running | TaskStatus::Interrupting
            ) && claims_overlap(&task.claims, &other.claims)
            {
                task.status = TaskStatus::Queued;
                task.updated_at = now();
                self.store.update_task(&task)?;
                return Ok(Some(task));
            }
        }

        // Budget concurrent
        let ws = self
            .store
            .get_workspace(&task.workspace)?
            .ok_or_else(|| OrqError::WorkspaceNotFound(task.workspace.clone()))?;
        let running = active
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Starting | TaskStatus::Running))
            .count() as u32;
        if running >= ws.max_concurrent {
            return Ok(Some(task));
        }

        // Acquire claim leases on paths table
        for claim in &task.claims {
            // ensure path poi exists
            let _ = self.store.set_poi(
                &task.workspace,
                "paths",
                claim,
                json!({ "glob": claim }),
                Default::default(),
                Some("claimed"),
                StorageTier::Ephemeral,
                task.session.as_deref(),
                None,
                Some(&task.id),
            );
            self.store.acquire_lease(
                &task.workspace,
                "paths",
                claim,
                LeaseKind::Write,
                &task.id,
                &format!("claim for task {}", task.name),
                3600,
            )?;
        }

        // Pre-exec triggers
        let engine = TriggerEngine::new(self.store);
        let mut seen = HashSet::new();
        if let Err(e) = engine.process_event(
            &task.workspace,
            task.session.as_deref(),
            "task.pre-exec",
            &json!({ "id": task.id, "name": task.name }),
            0,
            &mut seen,
        ) {
            task.status = TaskStatus::Blocked;
            task.updated_at = now();
            self.store.update_task(&task)?;
            self.store.release_leases_for_holder(&task.id)?;
            return Err(e);
        }

        task.status = TaskStatus::Starting;
        task.attempt += 1;
        task.updated_at = now();
        self.store.update_task(&task)?;

        Ok(Some(task))
    }

    pub fn spawn_process(&self, task: &mut Task) -> Result<Child> {
        let log_path = task
            .log_path
            .clone()
            .unwrap_or_else(|| self.store.log_path_for(&task.id).display().to_string());
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let err_file = log_file.try_clone()?;

        let cmd_line = resolve_profile(&task.profile, &task.command);
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.args(["/C", &cmd_line]);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.args(["-c", &cmd_line]);
            c
        };
        if let Some(ref cwd) = task.cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(err_file));

        let child = cmd.spawn()?;
        task.pid = Some(child.id());
        task.status = TaskStatus::Running;
        task.updated_at = now();
        self.store.update_task(task)?;
        self.store.append_event(
            &task.workspace,
            task.session.as_deref(),
            "task.started",
            json!({ "id": task.id, "pid": task.pid }),
        )?;
        Ok(child)
    }

    pub fn finish(
        &self,
        task: &mut Task,
        exit_code: Option<i32>,
        status: TaskStatus,
    ) -> Result<()> {
        task.exit_code = exit_code;
        task.status = status;
        task.pid = None;
        task.updated_at = now();
        self.store.update_task(task)?;
        self.store.release_leases_for_holder(&task.id)?;

        let kind = match status {
            TaskStatus::Done => "task.done",
            TaskStatus::Failed => "task.failed",
            TaskStatus::Cancelled => "task.cancelled",
            TaskStatus::Killed => "task.killed",
            _ => "task.updated",
        };
        self.store.append_event(
            &task.workspace,
            task.session.as_deref(),
            kind,
            json!({ "id": task.id, "exit_code": exit_code, "name": task.name }),
        )?;

        let engine = TriggerEngine::new(self.store);
        let mut seen = HashSet::new();
        let _ = engine.process_event(
            &task.workspace,
            task.session.as_deref(),
            "task.post-exec",
            &json!({ "id": task.id, "name": task.name, "exit_code": exit_code, "status": status.as_str() }),
            0,
            &mut seen,
        );
        let _ = engine.process_event(
            &task.workspace,
            task.session.as_deref(),
            kind,
            &json!({ "id": task.id, "name": task.name, "exit_code": exit_code }),
            0,
            &mut seen,
        );

        // Affinity feedback for standalone (non-job) model tasks; jobs finalize in JobEngine
        if task.job_id.is_none() {
            if let (Some(mid), Some(class)) = (task.model_id.as_deref(), task.class.as_deref()) {
                let success = status == TaskStatus::Done;
                let quality = self
                    .store
                    .get_poi(&task.workspace, "routing", "quality")
                    .ok()
                    .flatten()
                    .and_then(|p| p.value.as_f64());
                let _ = AffinityEngine::new(self.store).observe_outcome(
                    &task.workspace,
                    class,
                    mid,
                    success,
                    quality,
                );
            }
        } else if let Some(ref jid) = task.job_id {
            let _ = JobEngine::new(self.store).tick_job(jid);
        }

        // Service restart
        if task.kind == TaskKind::Service {
            let should = match task.restart {
                RestartPolicy::Always => true,
                RestartPolicy::OnFailure => status == TaskStatus::Failed,
                RestartPolicy::Never => false,
            };
            if should && !matches!(status, TaskStatus::Cancelled | TaskStatus::Killed) {
                let mut next = task.clone();
                next.id = new_id();
                next.status = TaskStatus::Queued;
                next.pid = None;
                next.exit_code = None;
                next.attempt = 0;
                next.created_at = now();
                next.updated_at = now();
                next.log_path = Some(self.store.log_path_for(&next.id).display().to_string());
                self.store.insert_task(&next)?;
            }
        } else if status == TaskStatus::Failed && task.attempt < task.max_attempts {
            let mut next = task.clone();
            next.id = new_id();
            next.status = TaskStatus::Queued;
            next.pid = None;
            next.exit_code = None;
            next.created_at = now();
            next.updated_at = now();
            next.log_path = Some(self.store.log_path_for(&next.id).display().to_string());
            // keep attempt count on new? reset for clarity — use same attempt chain via name
            self.store.insert_task(&next)?;
        }
        Ok(())
    }

    pub fn cancel(&self, task_id: &str, kill: bool) -> Result<Task> {
        let mut task = self
            .store
            .get_task(task_id)?
            .ok_or_else(|| OrqError::TaskNotFound(task_id.into()))?;
        if task.status.is_terminal() {
            return Ok(task);
        }
        if let Some(pid) = task.pid {
            terminate_pid(pid, kill);
        }
        let status = if kill {
            TaskStatus::Killed
        } else {
            TaskStatus::Cancelled
        };
        self.finish(&mut task, None, status)?;
        Ok(task)
    }

    pub fn interrupt(&self, task_id: &str) -> Result<Task> {
        let mut task = self
            .store
            .get_task(task_id)?
            .ok_or_else(|| OrqError::TaskNotFound(task_id.into()))?;
        if let Some(pid) = task.pid {
            // Send CTRL_BREAK / SIGINT equivalent — on Windows use taskkill soft
            interrupt_pid(pid);
        }
        task.status = TaskStatus::Interrupting;
        task.updated_at = now();
        self.store.update_task(&task)?;
        self.store.append_event(
            &task.workspace,
            task.session.as_deref(),
            "task.interrupt",
            json!({ "id": task.id }),
        )?;
        Ok(task)
    }

    pub fn await_task(&self, task_id: &str, timeout_ms: Option<u64>) -> Result<Task> {
        let start = std::time::Instant::now();
        loop {
            let task = self
                .store
                .get_task(task_id)?
                .ok_or_else(|| OrqError::TaskNotFound(task_id.into()))?;
            if task.status.is_terminal() {
                return Ok(task);
            }
            if let Some(ms) = timeout_ms {
                if start.elapsed().as_millis() as u64 > ms {
                    return Err(OrqError::Other(format!("await timeout for {task_id}")));
                }
            }
            // Drive scheduler one tick for sync mode
            let _ = self.tick_once(&task.workspace);
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// One scheduler tick: start queued tasks, reap finished children tracked externally.
    /// For sync mode without daemon, runs processes inline to completion.
    pub fn tick_once(&self, workspace: &str) -> Result<usize> {
        let queued: Vec<Task> = self
            .store
            .list_active_tasks(workspace)?
            .into_iter()
            .filter(|t| matches!(t.status, TaskStatus::Queued | TaskStatus::Blocked))
            .collect();
        let mut started = 0;
        for t in queued {
            if let Some(mut task) = self.try_start(&t.id)? {
                if task.status == TaskStatus::Starting {
                    // Run sync for simplicity in tick without external child map
                    match self.run_sync(&mut task) {
                        Ok(()) => started += 1,
                        Err(e) => {
                            let _ = self.finish(&mut task, Some(1), TaskStatus::Failed);
                            let _ = writeln!(
                                std::fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(task.log_path.as_deref().unwrap_or("orq.log"))
                                    .unwrap_or_else(|_| {
                                        // ignore
                                        std::fs::File::create("orq-fallback.log").unwrap()
                                    }),
                                "error: {e}"
                            );
                        }
                    }
                }
            }
        }
        Ok(started)
    }

    pub fn run_sync(&self, task: &mut Task) -> Result<()> {
        let log_path = task
            .log_path
            .clone()
            .unwrap_or_else(|| self.store.log_path_for(&task.id).display().to_string());
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let err_file = log_file.try_clone()?;
        let cmd_line = resolve_profile(&task.profile, &task.command);
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.args(["/C", &cmd_line]);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.args(["-c", &cmd_line]);
            c
        };
        if let Some(ref cwd) = task.cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(err_file));
        task.status = TaskStatus::Running;
        task.updated_at = now();
        self.store.update_task(task)?;
        let status = cmd.status()?;
        let code = status.code();
        let terminal = if status.success() {
            TaskStatus::Done
        } else {
            TaskStatus::Failed
        };
        self.finish(task, code, terminal)?;
        Ok(())
    }

    pub fn run_to_completion(&self, task_id: &str) -> Result<Task> {
        let _ = self.try_start(task_id)?;
        let mut task = self
            .store
            .get_task(task_id)?
            .ok_or_else(|| OrqError::TaskNotFound(task_id.into()))?;
        if task.status == TaskStatus::Starting || task.status == TaskStatus::Queued {
            if task.status == TaskStatus::Queued {
                let _ = self.try_start(task_id)?;
                task = self.store.get_task(task_id)?.unwrap();
            }
            if task.status == TaskStatus::Starting || task.status == TaskStatus::Running {
                self.run_sync(&mut task)?;
            }
        }
        self.store
            .get_task(task_id)?
            .ok_or_else(|| OrqError::TaskNotFound(task_id.into()))
    }
}

fn split_poi_ref(s: &str) -> (&str, &str) {
    if let Some((t, k)) = s.split_once('/') {
        (t, k)
    } else {
        ("default", s)
    }
}

fn resolve_profile(profile: &str, command: &str) -> String {
    match profile {
        "shell" | "" => command.to_string(),
        "cursor-agent" => format!("cursor agent {command}"),
        other => {
            // template: {cmd} placeholder
            if other.contains("{cmd}") {
                other.replace("{cmd}", command)
            } else {
                command.to_string()
            }
        }
    }
}

fn terminate_pid(pid: u32, force: bool) {
    #[cfg(windows)]
    {
        let args = if force {
            vec!["/F".to_string(), "/PID".into(), pid.to_string()]
        } else {
            vec!["/PID".into(), pid.to_string()]
        };
        let _ = Command::new("taskkill").args(&args).output();
    }
    #[cfg(unix)]
    {
        let sig = if force { 9 } else { 15 };
        let _ = Command::new("kill")
            .args(["-s", &sig.to_string(), &pid.to_string()])
            .output();
    }
}

fn interrupt_pid(pid: u32) {
    #[cfg(windows)]
    {
        // Best-effort: CTRL_BREAK not easily available; use taskkill without /F
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .output();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .output();
    }
}

/// Background daemon loop state
pub struct DaemonState {
    pub children: Mutex<Vec<(String, Child)>>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            children: Mutex::new(Vec::new()),
        }
    }
}

pub fn run_daemon_loop(store: Arc<Store>, state: Arc<DaemonState>, stop: Arc<Mutex<bool>>) {
    while !*stop.lock().unwrap() {
        // Reap children
        {
            let mut kids = state.children.lock().unwrap();
            let mut finished = Vec::new();
            for (i, (id, child)) in kids.iter_mut().enumerate() {
                if let Ok(Some(status)) = child.try_wait() {
                    finished.push((i, id.clone(), status.code()));
                }
            }
            for (offset, (i, id, code)) in finished.into_iter().enumerate() {
                kids.remove(i - offset);
                let engine = TaskEngine::new(&store);
                if let Ok(Some(mut task)) = store.get_task(&id) {
                    let terminal = if code.unwrap_or(1) == 0 {
                        TaskStatus::Done
                    } else {
                        TaskStatus::Failed
                    };
                    let _ = engine.finish(&mut task, code, terminal);
                }
            }
        }

        // Start queued across all workspaces
        if let Ok(workspaces) = store.list_workspaces() {
            for ws in workspaces {
                let engine = TaskEngine::new(&store);
                let active = store.list_active_tasks(&ws.name).unwrap_or_default();
                for t in active
                    .into_iter()
                    .filter(|t| matches!(t.status, TaskStatus::Queued | TaskStatus::Blocked))
                {
                    if let Ok(Some(mut task)) = engine.try_start(&t.id) {
                        if task.status == TaskStatus::Starting {
                            if let Ok(child) = engine.spawn_process(&mut task) {
                                state.children.lock().unwrap().push((task.id.clone(), child));
                            } else {
                                let _ = engine.finish(&mut task, Some(1), TaskStatus::Failed);
                            }
                        }
                    }
                }
                // Progress multi-model jobs (race / moa)
                if let Ok(jobs) = store.list_jobs(&ws.name, 50) {
                    let jing = JobEngine::new(&store);
                    for j in jobs {
                        if !j.status.is_terminal() {
                            let _ = jing.tick_job(&j.id);
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}
