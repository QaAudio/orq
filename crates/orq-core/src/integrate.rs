use crate::error::{OrqError, Result};
use std::path::Path;

pub const SKILL_MD: &str = r##"---
name: porq
description: Progressive multi-agent orchestration via the porq CLI — workspaces, POIs, canvases, supervised tasks, triggers, model affinities, and MoA merge jobs. Use when coordinating agents, locking paths/state, publishing dashboard canvases, routing models, or multi-model reconciliation.
---

# porq — progressive orchestration

## When to use
- Multi-agent workflows that need shared lockable state (POIs)
- Path claims to avoid edit conflicts
- Dashboard canvases (markdown / image / url / html) as the primary shared view
- Triggers that cancel/spawn remediation tasks
- Model routing / affinities / Mixture-of-Agents merge
- Ephemeral session work (prefer tmp tier + `--session`)

## Cheap surfaces (token discipline)
1. `porq status --json --limit 20` — task board snapshot
2. `porq poi ls <table> --json --limit 20` — POI table
3. `porq canvas ls --json` — dashboard canvases
4. `porq events --json --limit 30` — what happened
5. `porq job report <id>` / `porq report --md` — timelines

Never dump unbounded logs; use `--limit` / `--fields`.

## Zero-setup
First command auto-creates the `default` workspace. Daemon auto-spawns on supervised runs.
Prefer ephemeral:

```bash
porq --session "$ORQ_SESSION" poi table create notes --cols "body:string:poi"
porq --session "$ORQ_SESSION" run --sync --name work -- "echo hello"
porq --session "$ORQ_SESSION" gc
```

## Core primitives
- **Workspace** — sandbox (`--workspace`, default `default`)
- **POI** — lockable record in a table (`poi get/set/lock/steal`)
- **Canvas** — display POI in reserved `canvas` table (`canvas set/ls/rm`; kinds: markdown/image/url/html). Primary dashboard view.
- **Task** — supervised agent/process (`run/await/cancel/kill`)
- **Trigger** — declarative rules (`trigger add` …)
- **Model / Affinity / Job** — eval → seeded route → single|race|moa

## Canvases (dashboard primary view)
```bash
porq canvas set plan --md ./plan.md
porq canvas set shot --image ./out.png
porq dash serve --port 9847
```

Ops panels (board/tasks/events) live under the **Details** tab; Canvases is the default view.

## Model routing + MoA
```bash
porq model add fast --cli "echo FAST:{cmd}" --capability code
porq model add strong --cli "echo STRONG:{cmd}" --capability code
porq affinity set code.edit strong --score 0.9
porq run --sync --class code.edit --strategy moa --moa-k 2 --moa-aggregator strong --seed 42 --name edit -- "propose fix"
porq job report <id>
```

## CAS + claims
```bash
porq poi set roadmap item-1 '{"title":"x"}' --if-version 3
porq run --claim "src/engine/**" --name edit -- "…"
```

## Recipes
See `recipes/`: linear-sync, central-committer, review-gate, preland-gate, queue-drain, model-routing, moa-merge, review-agent, roadmap-sanity.

Env vars still use the `ORQ_*` prefix (compat); binary is `porq`.
"##;

pub const AGENTS_SNIPPET: &str = r#"
## porq (progressive orchestration)

When coordinating multiple agents or shared lockable state, prefer the `porq` CLI
(skill: `.cursor/skills/porq/SKILL.md` if installed via `porq integrate cursor`).

- Use `--json --limit` for machine-readable, token-bounded output.
- Prefer `--session` + ephemeral POI tier for short-lived agent work; `porq gc` when done.
- Path edits: declare `--claim "glob/**"` so the scheduler serializes overlapping work.
- Do not invent parallel lock files; use `porq poi lock` / claims.
- Share status on the dashboard with `porq canvas set` (primary view); Details holds ops tables.
- Multi-model work: register models, set affinities, prefer `--strategy single|race|moa --sync`.
"#;

/// Integration pack: install host-specific agent guidance without baking vendor logic into porq core.
pub trait IntegrationPack {
    fn id(&self) -> &'static str;
    fn integrate(&self, host_root: &Path) -> Result<Vec<String>>;
}

pub struct CursorPack;

impl IntegrationPack for CursorPack {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn integrate(&self, host_root: &Path) -> Result<Vec<String>> {
        integrate_cursor(host_root)
    }
}

/// Fixture pack used by conformance tests — writes a marker file only.
pub struct FakePack;

impl IntegrationPack for FakePack {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn integrate(&self, host_root: &Path) -> Result<Vec<String>> {
        let dir = host_root.join(".porq-integrate-fake");
        std::fs::create_dir_all(&dir)?;
        let marker = dir.join("OK");
        std::fs::write(&marker, "fake-pack-ok\n")?;
        Ok(vec![marker.display().to_string()])
    }
}

pub fn list_integration_packs() -> Vec<&'static str> {
    vec!["cursor", "fake"]
}

pub fn integrate_pack(target: &str, host_root: &Path) -> Result<Vec<String>> {
    match target {
        "cursor" => CursorPack.integrate(host_root),
        "fake" => FakePack.integrate(host_root),
        other => Err(OrqError::Other(format!(
            "unsupported integrate target: {other} (supported: {})",
            list_integration_packs().join(", ")
        ))),
    }
}

pub fn integrate_cursor(host_root: &Path) -> Result<Vec<String>> {
    let mut written = Vec::new();
    let skill_dir = host_root.join(".cursor/skills/porq");
    std::fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_path, SKILL_MD)?;
    written.push(skill_path.display().to_string());

    let agents = host_root.join("AGENTS.md");
    if agents.exists() {
        let existing = std::fs::read_to_string(&agents)?;
        if !existing.contains("## porq (progressive orchestration)") {
            let mut out = existing;
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(AGENTS_SNIPPET);
            std::fs::write(&agents, out)?;
            written.push(format!("{} (appended)", agents.display()));
        } else {
            written.push(format!("{} (already present)", agents.display()));
        }
    } else {
        std::fs::write(
            &agents,
            format!("# Agent notes\n{}", AGENTS_SNIPPET),
        )?;
        written.push(agents.display().to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fake_pack_writes_marker() {
        let dir = tempdir().unwrap();
        let written = integrate_pack("fake", dir.path()).unwrap();
        assert!(written[0].contains("OK"));
        assert!(dir.path().join(".porq-integrate-fake/OK").is_file());
    }
}
