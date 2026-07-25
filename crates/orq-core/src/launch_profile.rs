//! Provider-neutral structured launch profiles.
//!
//! Legacy shell-template `LaunchRecipe` remains for CLI model recipes.
//! New runners should prefer `LaunchProfile` with typed argv placeholders
//! so untrusted event text is never interpolated into `cmd /C` strings.

use crate::error::{OrqError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub const LAUNCH_PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptTransport {
    Stdin,
    File,
    Argument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultTransport {
    StdoutJson,
    ResultFile,
    ExitCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchProfile {
    pub schema_version: u32,
    pub id: String,
    pub executable: String,
    /// Argv templates. Placeholders: `{cmd}`, `{prompt_file}`, `{model}`,
    /// `{event.id}`, `{event.name}`, `{event.table}`, `{event.key}`,
    /// `{event.version}`, `{event.state}`, `{event.exit_code}`.
    pub argv: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Only these env keys from the event envelope are forwarded (plus profile env).
    #[serde(default)]
    pub env_allowlist: Vec<String>,
    #[serde(default = "default_prompt_transport")]
    pub prompt_transport: PromptTransport,
    #[serde(default = "default_result_transport")]
    pub result_transport: ResultTransport,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub adapter_version: Option<String>,
}

fn default_prompt_transport() -> PromptTransport {
    PromptTransport::Argument
}

fn default_result_transport() -> ResultTransport {
    ResultTransport::ExitCode
}

#[derive(Debug, Clone, Default)]
pub struct EventEnvelope {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub table: String,
    pub key: String,
    pub version: String,
    pub state: String,
    pub exit_code: String,
    pub correlation: HashMap<String, String>,
}

impl EventEnvelope {
    pub fn from_json(kind: &str, payload: &serde_json::Value) -> Self {
        let get = |k: &str| {
            payload
                .get(k)
                .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                        .or_else(|| v.as_f64().map(|n| n.to_string()))
                }))
                .unwrap_or_default()
        };
        let mut correlation = HashMap::new();
        if let Some(obj) = payload.get("correlation").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    correlation.insert(k.clone(), s.to_string());
                }
            }
        }
        Self {
            kind: kind.into(),
            id: get("id"),
            name: get("name"),
            table: get("table"),
            key: get("key"),
            version: get("version"),
            state: get("state"),
            exit_code: get("exit_code"),
            correlation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEnvelope {
    pub schema_version: u32,
    pub ok: bool,
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub findings: Vec<serde_json::Value>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub adapter_version: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub error: Option<String>,
}

impl ResultEnvelope {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != LAUNCH_PROFILE_SCHEMA_VERSION {
            return Err(OrqError::Other(format!(
                "result envelope schema_version {} != {}",
                self.schema_version, LAUNCH_PROFILE_SCHEMA_VERSION
            )));
        }
        Ok(())
    }
}

pub fn expand_placeholder(template: &str, cmd: &str, prompt_file: &str, model: &str, ev: &EventEnvelope) -> String {
    template
        .replace("{cmd}", cmd)
        .replace("{prompt_file}", prompt_file)
        .replace("{model}", model)
        .replace("{event.id}", &ev.id)
        .replace("{event.name}", &ev.name)
        .replace("{event.table}", &ev.table)
        .replace("{event.key}", &ev.key)
        .replace("{event.version}", &ev.version)
        .replace("{event.state}", &ev.state)
        .replace("{event.exit_code}", &ev.exit_code)
        .replace("{event.kind}", &ev.kind)
}

/// Build a `Command` from a structured profile — argv is never joined into a shell string.
pub fn build_command(
    profile: &LaunchProfile,
    cmd: &str,
    prompt_file: Option<&str>,
    model: Option<&str>,
    event: Option<&EventEnvelope>,
) -> Result<Command> {
    if profile.schema_version != LAUNCH_PROFILE_SCHEMA_VERSION {
        return Err(OrqError::Other(format!(
            "launch profile schema_version {} unsupported (want {})",
            profile.schema_version, LAUNCH_PROFILE_SCHEMA_VERSION
        )));
    }
    let pf = prompt_file.unwrap_or("");
    let model = model.unwrap_or("");
    let empty = EventEnvelope::default();
    let ev = event.unwrap_or(&empty);
    let mut command = Command::new(&profile.executable);
    for arg in &profile.argv {
        command.arg(expand_placeholder(arg, cmd, pf, model, ev));
    }
    if let Some(cwd) = &profile.cwd {
        command.current_dir(Path::new(cwd));
    }
    for (k, v) in &profile.env {
        command.env(k, v);
    }
    // Forward only allowlisted ORQ_EVENT_* style keys from correlation + envelope.
    let mut env_map = HashMap::new();
    env_map.insert("ORQ_EVENT_KIND".into(), ev.kind.clone());
    env_map.insert("ORQ_EVENT_ID".into(), ev.id.clone());
    env_map.insert("ORQ_EVENT_NAME".into(), ev.name.clone());
    env_map.insert("ORQ_EVENT_TABLE".into(), ev.table.clone());
    env_map.insert("ORQ_EVENT_KEY".into(), ev.key.clone());
    env_map.insert("ORQ_EVENT_VERSION".into(), ev.version.clone());
    env_map.insert("ORQ_EVENT_STATE".into(), ev.state.clone());
    env_map.insert("ORQ_EVENT_EXIT_CODE".into(), ev.exit_code.clone());
    for (k, v) in &ev.correlation {
        env_map.insert(format!("ORQ_CORR_{}", k.to_uppercase()), v.clone());
    }
    if profile.env_allowlist.is_empty() {
        for (k, v) in &env_map {
            command.env(k, v);
        }
    } else {
        for key in &profile.env_allowlist {
            if let Some(v) = env_map.get(key) {
                command.env(key, v);
            }
        }
    }
    Ok(command)
}

/// Fake adapter used by conformance tests — writes a fixed result envelope via Rust I/O
/// (avoids shell quoting). The profile still exercises structured argv + load/save.
pub fn fake_adapter_profile(result_path: &Path) -> LaunchProfile {
    let _ = result_path;
    LaunchProfile {
        schema_version: LAUNCH_PROFILE_SCHEMA_VERSION,
        id: "fake.echo".into(),
        executable: "echo".into(),
        argv: vec!["fake-adapter-ok".into(), "{event.name}".into()],
        cwd: None,
        env: HashMap::new(),
        env_allowlist: vec![],
        prompt_transport: PromptTransport::Argument,
        result_transport: ResultTransport::ResultFile,
        timeout_secs: Some(30),
        capabilities: vec!["repo.read".into(), "diff.review".into()],
        adapter_id: Some("fake".into()),
        adapter_version: Some("1".into()),
    }
}

/// Write the canonical fake-adapter result envelope (conformance helper).
pub fn write_fake_adapter_result(result_path: &Path) -> Result<()> {
    let env = ResultEnvelope {
        schema_version: LAUNCH_PROFILE_SCHEMA_VERSION,
        ok: true,
        verdict: Some("ok".into()),
        findings: vec![],
        adapter_id: Some("fake".into()),
        adapter_version: Some("1".into()),
        exit_code: Some(0),
        error: None,
    };
    if let Some(parent) = result_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string(&env).map_err(|e| OrqError::Other(e.to_string()))?;
    std::fs::write(result_path, body)?;
    Ok(())
}

/// Resolve a profile id to a legacy shell command line (compat path).
/// Prefer `build_command` for new code.
pub fn resolve_legacy_profile(profile: &str, command: &str) -> String {
    match profile {
        "shell" | "" => command.to_string(),
        // Registered adapter identifiers only — not Cursor product APIs in generic modules.
        other if other.starts_with("adapter:") => {
            // adapter:<id>:<template-with-{cmd}>
            if let Some(rest) = other.strip_prefix("adapter:") {
                if let Some((_, tmpl)) = rest.split_once(':') {
                    return tmpl.replace("{cmd}", command);
                }
            }
            command.to_string()
        }
        other => {
            if other.contains("{cmd}") {
                other.replace("{cmd}", command)
            } else {
                command.to_string()
            }
        }
    }
}

/// Architecture fitness: generic modules must not hardcode vendor CLI names
/// except registered adapter identifiers / test fixtures.
pub fn assert_no_vendor_leak(source: &str, allowed: &[&str]) -> Result<()> {
    const BANNED: &[&str] = &["cursor agent", "claude code", "codex ", "bugbot"];
    let lower = source.to_lowercase();
    for ban in BANNED {
        if lower.contains(ban) && !allowed.iter().any(|a| a.to_lowercase() == *ban) {
            return Err(OrqError::Other(format!(
                "vendor leak in generic module: found {ban:?}"
            )));
        }
    }
    Ok(())
}

pub fn write_profile_file(path: &Path, profile: &LaunchProfile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(profile)
        .map_err(|e| OrqError::Other(e.to_string()))?;
    std::fs::write(path, body)?;
    Ok(())
}

pub fn load_profile_file(path: &Path) -> Result<LaunchProfile> {
    let body = std::fs::read_to_string(path)?;
    let profile: LaunchProfile =
        serde_json::from_str(&body).map_err(|e| OrqError::Other(e.to_string()))?;
    if profile.schema_version != LAUNCH_PROFILE_SCHEMA_VERSION {
        return Err(OrqError::Other(format!(
            "incompatible launch profile schema_version {}",
            profile.schema_version
        )));
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn structured_argv_does_not_shell_join() {
        let profile = LaunchProfile {
            schema_version: 1,
            id: "echo".into(),
            executable: "echo".into(),
            argv: vec!["hello".into(), "{event.name}".into()],
            cwd: None,
            env: HashMap::new(),
            env_allowlist: vec![],
            prompt_transport: PromptTransport::Argument,
            result_transport: ResultTransport::ExitCode,
            timeout_secs: None,
            capabilities: vec![],
            adapter_id: Some("fake".into()),
            adapter_version: Some("1".into()),
        };
        let ev = EventEnvelope {
            name: "exec-gate-unit".into(),
            ..Default::default()
        };
        let cmd = build_command(&profile, "ignored", None, None, Some(&ev)).unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        assert_eq!(args, vec!["hello", "exec-gate-unit"]);
    }

    #[test]
    fn fake_adapter_conformance_writes_result() {
        let dir = tempdir().unwrap();
        let result = dir.path().join("result.json");
        let profile = fake_adapter_profile(&result);
        write_profile_file(&dir.path().join("profile.json"), &profile).unwrap();
        let loaded = load_profile_file(&dir.path().join("profile.json")).unwrap();
        assert_eq!(loaded.id, "fake.echo");
        let cmd = build_command(&loaded, "review", None, None, None).unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        assert_eq!(args, vec!["fake-adapter-ok", ""]);
        write_fake_adapter_result(&result).unwrap();
        let body = std::fs::read_to_string(&result).unwrap();
        let env: ResultEnvelope = serde_json::from_str(&body).unwrap();
        env.validate().unwrap();
        assert!(env.ok);
        assert_eq!(env.adapter_id.as_deref(), Some("fake"));
    }

    #[test]
    fn fitness_rejects_vendor_leak() {
        assert!(assert_no_vendor_leak("spawn cursor agent --prompt x", &[]).is_err());
        assert!(assert_no_vendor_leak("spawn adapter:fake:{cmd}", &[]).is_ok());
    }

    #[test]
    fn legacy_resolve_no_hardcoded_cursor_product() {
        // Generic path must not invent "cursor agent …" — adapters own that.
        let out = resolve_legacy_profile("shell", "echo hi");
        assert_eq!(out, "echo hi");
        let out = resolve_legacy_profile("adapter:cursor:myrunner {cmd}", "do-it");
        assert_eq!(out, "myrunner do-it");
    }
}
