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
                match self.execute_action(workspace, session, action, rule.blocking, depth, seen) {
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
                let task = Task {
                    id: new_id(),
                    workspace: workspace.into(),
                    session: session.map(|s| s.to_string()),
                    name: name.clone().unwrap_or_else(|| "triggered".into()),
                    kind: task_kind,
                    status: TaskStatus::Queued,
                    command: command.clone(),
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
                    json!({ "id": task.id, "command": command }),
                )?;
            }
            TriggerAction::RunHook { command } => {
                let status = run_hook(command, HOOK_TIMEOUT_SECS)?;
                if !status.success() {
                    let msg = format!("hook failed: {command} (exit {:?})", status.code());
                    if blocking {
                        return Err(OrqError::TriggerVeto(msg));
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
    if let Some(rest) = cond.strip_prefix("key==") {
        let expected = rest.trim().trim_matches('"').trim_matches('\'');
        return payload
            .get("key")
            .and_then(|v| v.as_str())
            .map(|s| s == expected)
            .unwrap_or(false);
    }
    // empty condition = match all
    cond.is_empty() || cond == "*"
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

fn run_hook(command: &str, _timeout_secs: u64) -> Result<std::process::ExitStatus> {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    cmd.status().map_err(OrqError::from)
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
}
