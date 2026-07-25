use crate::error::{OrqError, Result};
use crate::store::Store;
use crate::types::*;
use serde_json::json;
use std::process::Command;

pub struct Evaluator<'a> {
    store: &'a Store,
}

impl<'a> Evaluator<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn evaluate(
        &self,
        workspace: &str,
        name: &str,
        command: &str,
        claims: &[String],
        needs_poi: &[String],
        class_override: Option<&str>,
    ) -> Result<EvalResult> {
        if let Some(c) = class_override {
            return Ok(EvalResult {
                class: c.into(),
                features: feature_json(name, command, claims, needs_poi),
            });
        }
        if let Some(hook) = self.store.get_eval_hook(workspace)? {
            if let Ok(r) = run_eval_hook(&hook, name, command, claims) {
                return Ok(r);
            }
        }
        Ok(default_eval(name, command, claims, needs_poi))
    }
}

fn feature_json(
    name: &str,
    command: &str,
    claims: &[String],
    needs_poi: &[String],
) -> serde_json::Value {
    json!({
        "name": name,
        "command_len": command.len(),
        "claims": claims,
        "needs_poi": needs_poi,
        "tokens": tokenize(&format!("{name} {command}")),
    })
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_' && c != '-')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

fn default_eval(
    name: &str,
    command: &str,
    claims: &[String],
    needs_poi: &[String],
) -> EvalResult {
    let blob = format!("{name} {command}").to_lowercase();
    let class = if blob.contains("review") || blob.contains("diff") {
        "review.diff"
    } else if blob.contains("research") || blob.contains("search") || blob.contains("qa") {
        "research.qa"
    } else if !claims.is_empty()
        || blob.contains("edit")
        || blob.contains("refactor")
        || blob.contains("fix")
        || blob.contains("implement")
        || blob.contains("code")
    {
        "code.edit"
    } else {
        "generic"
    };
    EvalResult {
        class: class.into(),
        features: feature_json(name, command, claims, needs_poi),
    }
}

fn run_eval_hook(
    hook_cmd: &str,
    name: &str,
    command: &str,
    claims: &[String],
) -> Result<EvalResult> {
    let input = json!({
        "name": name,
        "command": command,
        "claims": claims,
    });
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", hook_cmd]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", hook_cmd]);
        c
    };
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(input.to_string().as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(OrqError::Other("eval hook failed".into()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let class = v
        .get("class")
        .and_then(|c| c.as_str())
        .ok_or_else(|| OrqError::Other("eval hook missing class".into()))?
        .to_string();
    let features = v.get("features").cloned().unwrap_or(json!({}));
    Ok(EvalResult { class, features })
}
