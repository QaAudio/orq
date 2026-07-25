use crate::error::{OrqError, Result};
use crate::store::Store;
use crate::types::*;
use serde_json::json;
use std::collections::HashSet;
use std::process::Command;

const DEFAULT_CASCADE_DEPTH: u32 = 8;
const HOOK_TIMEOUT_SECS: u64 = 30;

pub struct TriggerEngine<'a> {
    store: &'a Store,
}

impl<'a> TriggerEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn add(
        &self,
        workspace: &str,
        name: &str,
        event_pattern: &str,
        condition: Option<&str>,
        actions: Vec<TriggerAction>,
        blocking: bool,
        max_fires_per_hour: u32,
    ) -> Result<TriggerRule> {
        let rule = TriggerRule {
            id: new_id(),
            workspace: workspace.into(),
            name: name.into(),
            event_pattern: event_pattern.into(),
            condition: condition.map(|s| s.to_string()),
            actions,
            blocking,
            max_fires_per_hour,
            enabled: true,
            created_at: now(),
        };
        self.store.insert_trigger(&rule)?;
        self.store.append_event(
            workspace,
            None,
            "trigger.created",
            json!({ "id": rule.id, "name": name, "pattern": event_pattern }),
        )?;
        Ok(rule)
    }

    /// Process event against all matching triggers.
    /// Returns Ok(()) or Err(TriggerVeto) if a blocking hook vetoes.
    pub fn process_event(
        &self,
        workspace: &str,
        session: Option<&str>,
        kind: &str,
        payload: &serde_json::Value,
        depth: u32,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if depth > DEFAULT_CASCADE_DEPTH {
            return Err(OrqError::CascadeDepth(depth));
        }
        let rules = self.store.list_triggers(workspace)?;
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            if !pattern_matches(&rule.event_pattern, kind) {
                continue;
            }
            if let Some(ref cond) = rule.condition {
                if !condition_matches(cond, payload) {
                    continue;
                }
            }
            let cycle_key = format!("{}:{}", rule.id, kind);
            if seen.contains(&cycle_key) {
                continue;
            }
            seen.insert(cycle_key);

            let fires = self.store.trigger_fires_last_hour(&rule.id)?;
            if fires >= rule.max_fires_per_hour {
                return Err(OrqError::BudgetExceeded(format!(
                    "trigger {} fired {} times in last hour",
                    rule.name, fires
                )));
            }

            self.store.record_trigger_fire(&rule.id)?;
            self.store.append_event(
                workspace,
                session,
                "trigger.fired",
                json!({
                    "trigger_id": rule.id,
                    "name": rule.name,
                    "event": kind,
                    "depth": depth,
                }),
            )?;

            for action in &rule.actions {
                match self.execute_action(
                    workspace,
                    session,
                    kind,
                    payload,
                    action,
                    rule.blocking,
                    depth,
                    seen,
                ) {
                    Ok(()) => {}
                    Err(e) if rule.blocking => return Err(e),
                    Err(e) => {
                        self.store.append_event(
                            workspace,
                            session,
                            "trigger.action_error",
                            json!({ "trigger_id": rule.id, "error": e.to_string() }),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_action(
        &self,
        workspace: &str,
        session: Option<&str>,
        event_kind: &str,
        payload: &serde_json::Value,
        action: &TriggerAction,
        blocking: bool,
        depth: u32,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        match action {
            TriggerAction::SetPoi {
                table,
                key,
                value,
                state,
            } => {
                let poi = self.store.set_poi(
                    workspace,
                    table,
                    key,
                    value.clone(),
                    Default::default(),
                    state.as_deref(),
                    StorageTier::Durable,
                    session,
                    None,
                    None,
                )?;
                // Nested event processing
                self.process_event(
                    workspace,
                    session,
                    "poi.changed",
                    &json!({
                        "table": table,
                        "key": key,
                        "version": poi.version,
                        "state": poi.state,
                        "value": poi.value,
                    }),
                    depth + 1,
                    seen,
                )?;
            }
            TriggerAction::CancelTasks { selector } => {
                let tasks = self.store.list_active_tasks(workspace)?;
                for mut t in tasks {
                    if !selector_matches(selector, &t) {
                        continue;
                    }
                    t.status = TaskStatus::Cancelled;
                    t.updated_at = now();
                    t.pid = None;
                    self.store.update_task(&t)?;
                    self.store.release_leases_for_holder(&t.id)?;
                    self.store.append_event(
                        workspace,
                        t.session.as_deref(),
                        "task.cancelled",
                        json!({ "id": t.id, "reason": "trigger", "selector": selector }),
                    )?;
                    self.process_event(
                        workspace,
                        session,
                        "task.cancelled",
                        &json!({ "id": t.id, "name": t.name }),
                        depth + 1,
                        seen,
                    )?;
                }
            }
            TriggerAction::SpawnTask {
                command,
                name,
                kind,
            } => {
                let ws = self
                    .store
                    .get_workspace(workspace)?
                    .ok_or_else(|| OrqError::WorkspaceNotFound(workspace.into()))?;
                let spawns = self.store.spawns_last_hour(workspace)?;
                if spawns >= ws.max_spawns_per_hour {
                    return Err(OrqError::BudgetExceeded(format!(
                        "workspace spawn budget {}/h",
                        ws.max_spawns_per_hour
                    )));
                }
                let task_kind = kind
                    .as_deref()
                    .and_then(TaskKind::parse)
                    .unwrap_or(TaskKind::Oneshot);
                let expanded_cmd = expand_event_template(command, event_kind, payload);
                let expanded_name = name
                    .as_ref()
                    .map(|n| expand_event_template(n, event_kind, payload))
                    .unwrap_or_else(|| "triggered".into());
                let command_with_env =
                    inject_event_env(&expanded_cmd, event_kind, payload);
                let task = Task {
                    id: new_id(),
                    workspace: workspace.into(),
                    session: session.map(|s| s.to_string()),
                    name: expanded_name,
                    kind: task_kind,
                    status: TaskStatus::Queued,
                    command: command_with_env,
                    cwd: Some(ws.root.clone()),
                    profile: "shell".into(),
                    claims: vec![],
                    depends_on: vec![],
                    needs_poi: vec![],
                    restart: if task_kind == TaskKind::Service {
                        RestartPolicy::OnFailure
                    } else {
                        RestartPolicy::Never
                    },
                    attempt: 0,
                    max_attempts: 1,
                    pid: None,
                    exit_code: None,
                    log_path: None,
                    job_id: None,
                    model_id: None,
                    class: None,
                    role: None,
                    created_at: now(),
                    updated_at: now(),
                };
                self.store.insert_task(&task)?;
                self.store.record_spawn(workspace)?;
                self.store.append_event(
                    workspace,
                    session,
                    "task.queued_by_trigger",
                    json!({
                        "id": task.id,
                        "command": task.command,
                        "event": event_kind,
                        "payload": payload,
                    }),
                )?;
            }
            TriggerAction::RunHook { command } => {
                let expanded = expand_event_template(command, event_kind, payload);
                match run_hook(&expanded, HOOK_TIMEOUT_SECS)? {
                    HookOutcome::Success(status) if status.success() => {}
                    HookOutcome::Success(status) => {
                        let msg = format!(
                            "hook failed: {expanded} (exit {:?})",
                            status.code()
                        );
                        if blocking {
                            return Err(OrqError::TriggerVeto(msg));
                        }
                    }
                    HookOutcome::Timeout => {
                        let msg = format!(
                            "hook timed out after {HOOK_TIMEOUT_SECS}s: {expanded}"
                        );
                        if blocking {
                            return Err(OrqError::TriggerVeto(msg));
                        }
                        self.store.append_event(
                            workspace,
                            session,
                            "trigger.action_error",
                            json!({ "error": msg }),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn pattern_matches(pattern: &str, kind: &str) -> bool {
    if pattern == "*" || pattern == kind {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return kind.starts_with(prefix);
    }
    // support poi.state==X style as event kind filter + condition
    if pattern.starts_with("poi.state==") {
        return kind == "poi.changed";
    }
    false
}

fn condition_matches(cond: &str, payload: &serde_json::Value) -> bool {
    let parts: Vec<&str> = cond
        .split("&&")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return cond.is_empty() || cond == "*";
    }
    parts.iter().all(|part| single_condition_matches(part, payload))
}

fn single_condition_matches(cond: &str, payload: &serde_json::Value) -> bool {
    if cond == "*" {
        return true;
    }
    if let Some(rest) = cond.strip_prefix("state==") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("state")
            .and_then(|v| v.as_str())
            .map(|s| s == expected)
            .unwrap_or(false);
    }
    if let Some(rest) = cond.strip_prefix("poi.state==") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("state")
            .and_then(|v| v.as_str())
            .map(|s| s == expected)
            .unwrap_or(false);
    }
    if let Some(rest) = cond.strip_prefix("name==") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s == expected)
            .unwrap_or(false);
    }
    if let Some(rest) = cond.strip_prefix("name^=") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with(expected))
            .unwrap_or(false);
    }
    if let Some(rest) = cond.strip_prefix("key==") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("key")
            .and_then(|v| v.as_str())
            .map(|s| s == expected)
            .unwrap_or(false);
    }
    if let Some(rest) = cond.strip_prefix("key^=") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("key")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with(expected))
            .unwrap_or(false);
    }
    if let Some(rest) = cond.strip_prefix("table==") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("table")
            .and_then(|v| v.as_str())
            .map(|s| s == expected)
            .unwrap_or(false);
    }
    false
}

fn selector_matches(selector: &str, task: &Task) -> bool {
    if selector == "*" {
        return true;
    }
    if let Some(prefix) = selector.strip_prefix("name:") {
        return task.name == prefix || task.name.contains(prefix);
    }
    if let Some(prefix) = selector.strip_prefix("tag:") {
        return task.name.contains(prefix);
    }
    task.name.contains(selector) || task.id.starts_with(selector)
}

fn payload_str(payload: &serde_json::Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| Some(v.to_string().trim_matches('"').to_string()))
        })
        .unwrap_or_default()
}

fn expand_event_template(template: &str, event_kind: &str, payload: &serde_json::Value) -> String {
    template
        .replace("{event}", event_kind)
        .replace("{id}", &payload_str(payload, "id"))
        .replace("{name}", &payload_str(payload, "name"))
        .replace("{table}", &payload_str(payload, "table"))
        .replace("{key}", &payload_str(payload, "key"))
        .replace("{version}", &payload_str(payload, "version"))
        .replace("{state}", &payload_str(payload, "state"))
        .replace("{exit_code}", &payload_str(payload, "exit_code"))
}

fn inject_event_env(command: &str, event_kind: &str, payload: &serde_json::Value) -> String {
    let pairs = [
        ("ORQ_EVENT", event_kind.to_string()),
        ("ORQ_EVENT_ID", payload_str(payload, "id")),
        ("ORQ_EVENT_NAME", payload_str(payload, "name")),
        ("ORQ_EVENT_TABLE", payload_str(payload, "table")),
        ("ORQ_EVENT_KEY", payload_str(payload, "key")),
        ("ORQ_EVENT_VERSION", payload_str(payload, "version")),
        ("ORQ_EVENT_STATE", payload_str(payload, "state")),
        ("ORQ_EVENT_EXIT_CODE", payload_str(payload, "exit_code")),
    ];
    #[cfg(windows)]
    {
        let mut prefix = String::new();
        for (k, v) in pairs {
            if v.is_empty() {
                continue;
            }
            // Escape quotes in values for cmd set
            let safe = v.replace('"', "");
            prefix.push_str(&format!("set \"{k}={safe}\"&& "));
        }
        format!("{prefix}{command}")
    }
    #[cfg(not(windows))]
    {
        let mut prefix = String::new();
        for (k, v) in pairs {
            if v.is_empty() {
                continue;
            }
            let safe = v.replace('\'', "'\\''");
            prefix.push_str(&format!("{k}='{safe}' "));
        }
        format!("{prefix}{command}")
    }
}

enum HookOutcome {
    Success(std::process::ExitStatus),
    Timeout,
}

fn run_hook(command: &str, timeout_secs: u64) -> Result<HookOutcome> {
    #[cfg(windows)]
    let mut child = {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c.spawn().map_err(OrqError::from)?
    };
    #[cfg(not(windows))]
    let mut child = {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c.spawn().map_err(OrqError::from)?
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait().map_err(OrqError::from)? {
            Some(status) => return Ok(HookOutcome::Success(status)),
            None if std::time::Instant::now() >= deadline => {
                let pid = child.id();
                terminate_process_tree(pid);
                let _ = child.kill();
                let _ = child.wait();
                return Ok(HookOutcome::Timeout);
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

fn terminate_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-9", &format!("-{pid}")])
            .output();
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use tempfile::tempdir;

    #[test]
    fn poi_change_cancels_and_spawns_remediation() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.ensure_workspace("default", None).unwrap();
        store
            .create_poi_table(
                "default",
                "health",
                "generic",
                vec![ColumnDef {
                    name: "status".into(),
                    col_type: "string".into(),
                    poi: true,
                }],
            )
            .unwrap();

        // seed a running-like task
        let mut victim = Task {
            id: new_id(),
            workspace: "default".into(),
            session: None,
            name: "worker".into(),
            kind: TaskKind::Oneshot,
            status: TaskStatus::Running,
            command: "echo victim".into(),
            cwd: None,
            profile: "shell".into(),
            claims: vec![],
            depends_on: vec![],
            needs_poi: vec![],
            restart: RestartPolicy::Never,
            attempt: 1,
            max_attempts: 1,
            pid: None,
            exit_code: None,
            log_path: None,
            job_id: None,
            model_id: None,
            class: None,
            role: None,
            created_at: now(),
            updated_at: now(),
        };
        store.insert_task(&victim).unwrap();

        let engine = TriggerEngine::new(&store);
        engine
            .add(
                "default",
                "on-broken",
                "poi.changed",
                Some("state==broken"),
                vec![TriggerAction::CancelTasks {
                    selector: "name:worker".into(),
                }],
                false,
                60,
            )
            .unwrap();
        engine
            .add(
                "default",
                "on-cancel-remediate",
                "task.cancelled",
                None,
                vec![TriggerAction::SpawnTask {
                    command: "echo remediate".into(),
                    name: Some("remediation".into()),
                    kind: None,
                }],
                false,
                60,
            )
            .unwrap();

        store
            .set_poi(
                "default",
                "health",
                "system",
                json!("bad"),
                Default::default(),
                Some("broken"),
                StorageTier::Durable,
                None,
                None,
                None,
            )
            .unwrap();

        let mut seen = HashSet::new();
        engine
            .process_event(
                "default",
                None,
                "poi.changed",
                &json!({"table":"health","key":"system","state":"broken"}),
                0,
                &mut seen,
            )
            .unwrap();

        victim = store.get_task(&victim.id).unwrap().unwrap();
        assert_eq!(victim.status, TaskStatus::Cancelled);

        let tasks = store.list_tasks("default", None, 50).unwrap();
        assert!(tasks.iter().any(|t| t.name == "remediation"));
    }

    #[test]
    fn condition_prefix_and_table_and_combo() {
        let payload = json!({
            "name": "exec-gate-u1",
            "key": "proposal-42",
            "table": "roadmap-proposals",
            "state": "proposed"
        });
        assert!(condition_matches("name^=exec-", &payload));
        assert!(!condition_matches("name^=review-", &payload));
        assert!(condition_matches("key^=proposal-", &payload));
        assert!(condition_matches("table==roadmap-proposals", &payload));
        assert!(condition_matches(
            "table==roadmap-proposals && state==proposed",
            &payload
        ));
        assert!(!condition_matches(
            "table==roadmap-proposals && state==approved",
            &payload
        ));
    }

    #[test]
    fn expand_event_template_fills_fields() {
        let out = expand_event_template(
            "echo {event} {id} {name} {table}/{key}",
            "task.done",
            &json!({"id":"t1","name":"exec-a","table":"board","key":"k1"}),
        );
        assert_eq!(out, "echo task.done t1 exec-a board/k1");
    }
}
