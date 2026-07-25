use crate::error::Result;
use std::path::Path;

pub const SKILL_MD: &str = r##"---
name: orq
description: Multi-agent task orchestration via the orq CLI — workspaces, POIs, canvases, supervised tasks, triggers, model affinities, and MoA merge jobs. Use when coordinating agents, locking paths/state, publishing dashboard canvases, routing models, or multi-model reconciliation.
---

# orq — multi-agent orchestrator

## When to use
- Multi-agent workflows that need shared lockable state (POIs)
- Path claims to avoid edit conflicts
- Triggers that cancel/spawn remediation tasks
- Model routing / affinities / Mixture-of-Agents merge
- Ephemeral session work (prefer tmp tier + `--session`)

## Cheap surfaces (token discipline)
1. `orq status --json --limit 20` — task board snapshot
2. `orq poi ls <table> --json --limit 20` — POI table
3. `orq events --json --limit 30` — what happened
4. `orq job report <id>` / `orq report --md` — timelines
5. `orq affinity ls --class code.edit --json` — routing scores

Never dump unbounded logs; use `--limit` / `--fields`.

## Zero-setup
First command auto-creates the `default` workspace. Daemon auto-spawns on supervised runs.
Prefer ephemeral:

```bash
orq --session "$ORQ_SESSION" poi table create notes --cols "body:string:poi"
orq --session "$ORQ_SESSION" run --sync --name work -- "echo hello"
orq --session "$ORQ_SESSION" gc
```

## Core primitives
- **Workspace** — sandbox (`--workspace`, default `default`)
- **POI** — lockable record in a table (`poi get/set/lock/steal`)
- **Canvas** — display POI in reserved `canvas` table (`canvas set/ls/rm`; kinds: markdown/image/url/html)
- **Task** — supervised agent/process (`run/await/cancel/kill`)
- **Trigger** — declarative rules (`trigger add` …)
- **Model / Affinity / Job** — eval → seeded route → single|race|moa

## Canvases (dashboard display)
```bash
orq canvas set plan --md ./plan.md
orq canvas set shot --image ./out.png
orq dash serve --port 9847
```

## Model routing + MoA
```bash
orq model add fast --cli "echo FAST:{cmd}" --capability code
orq model add strong --cli "echo STRONG:{cmd}" --capability code
orq affinity set code.edit strong --score 0.9
orq run --sync --class code.edit --strategy moa --moa-k 2 --moa-aggregator strong --seed 42 --name edit -- "propose fix"
orq job report <id>
```

## CAS + claims
```bash
orq poi set roadmap item-1 '{"title":"x"}' --if-version 3
orq run --claim "src/engine/**" --name edit -- "…"
```

## Recipes
See `recipes/`: linear-sync, central-committer, review-gate, preland-gate, queue-drain, model-routing, moa-merge.
"##;

pub const AGENTS_SNIPPET: &str = r#"
## orq (multi-agent orchestration)

When coordinating multiple agents or shared lockable state, prefer the `orq` CLI
(skill: `.cursor/skills/orq/SKILL.md` if installed via `orq integrate cursor`).

- Use `--json --limit` for machine-readable, token-bounded output.
- Prefer `--session` + ephemeral POI tier for short-lived agent work; `orq gc` when done.
- Path edits: declare `--claim "glob/**"` so the scheduler serializes overlapping work.
- Do not invent parallel lock files; use `orq poi lock` / claims.
- Multi-model work: register models, set affinities, prefer `--strategy single|race|moa --sync`.
"#;

pub fn integrate_cursor(host_root: &Path) -> Result<Vec<String>> {
    let mut written = Vec::new();
    let skill_dir = host_root.join(".cursor/skills/orq");
    std::fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_path, SKILL_MD)?;
    written.push(skill_path.display().to_string());

    let agents = host_root.join("AGENTS.md");
    if agents.exists() {
        let existing = std::fs::read_to_string(&agents)?;
        if !existing.contains("## orq (multi-agent orchestration)") {
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
