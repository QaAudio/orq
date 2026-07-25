use crate::error::Result;
use crate::store::Store;
use serde_json::json;

pub fn render_markdown(store: &Store, workspace: &str, limit: usize) -> Result<String> {
    let tasks = store.list_tasks(workspace, None, limit)?;
    let events = store.list_events(workspace, None, limit.min(100), None)?;
    let tables = store.list_poi_tables(workspace)?;
    let triggers = store.list_triggers(workspace)?;

    let mut md = String::new();
    md.push_str(&format!("# orq report — `{workspace}`\n\n"));
    md.push_str("## Tasks\n\n");
    md.push_str("| id | name | status | kind | exit |\n|---|---|---|---|---|\n");
    for t in &tasks {
        md.push_str(&format!(
            "| `{}` | {} | {} | {} | {:?} |\n",
            short(&t.id),
            t.name,
            t.status.as_str(),
            t.kind.as_str(),
            t.exit_code
        ));
    }
    md.push_str("\n## Triggers\n\n");
    for tr in &triggers {
        md.push_str(&format!(
            "- **{}** `{}` blocking={} enabled={}\n",
            tr.name, tr.event_pattern, tr.blocking, tr.enabled
        ));
    }
    md.push_str("\n## POI tables\n\n");
    for t in &tables {
        let pois = store.list_pois(workspace, &t.name, None, 20)?;
        md.push_str(&format!("### {} ({})\n\n", t.name, t.table_type));
        for p in pois {
            md.push_str(&format!(
                "- `{}` v{} state=`{}` blocked={}\n",
                p.key, p.version, p.state, p.blocked
            ));
        }
        md.push('\n');
    }
    md.push_str("## Recent events\n\n");
    for e in events.iter().rev().take(30) {
        md.push_str(&format!(
            "- `{}` **{}** {}\n",
            e.id,
            e.kind,
            e.payload
        ));
    }
    Ok(md)
}

pub fn render_html(store: &Store, workspace: &str, limit: usize) -> Result<String> {
    let md = render_markdown(store, workspace, limit)?;
    // Minimal HTML wrap — no markdown crate dependency
    let escaped = html_escape(&md);
    Ok(format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>orq report {workspace}</title>
<style>
body {{ font-family: ui-monospace, Consolas, monospace; background:#0d1117; color:#e6edf3; padding:1.5rem; }}
pre {{ white-space: pre-wrap; }}
</style></head><body><pre>{escaped}</pre></body></html>"#
    ))
}

pub fn status_json(store: &Store, workspace: &str, session: Option<&str>, limit: usize) -> Result<serde_json::Value> {
    Ok(json!({
        "workspace": workspace,
        "session": session,
        "tasks": store.list_tasks(workspace, session, limit)?,
        "triggers": store.list_triggers(workspace)?,
        "tables": store.list_poi_tables(workspace)?,
    }))
}

fn short(id: &str) -> &str {
    if id.len() > 8 {
        &id[..8]
    } else {
        id
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
