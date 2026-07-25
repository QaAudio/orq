use crate::error::{OrqError, Result};
use std::path::Path;

/// Cursor skill body — edited under `templates/cursor/SKILL.md`.
pub const SKILL_MD: &str = include_str!("../../../templates/cursor/SKILL.md");

/// AGENTS.md append block — edited under `templates/cursor/AGENTS.snippet.md`.
pub const AGENTS_SNIPPET: &str = include_str!("../../../templates/cursor/AGENTS.snippet.md");

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
        assert!(written[0].ends_with("OK"));
        assert!(dir.path().join(".porq-integrate-fake/OK").is_file());
    }

    #[test]
    fn skill_template_is_nonempty() {
        assert!(SKILL_MD.contains("porq"));
        assert!(SKILL_MD.contains("Cheap surfaces"));
        assert!(AGENTS_SNIPPET.contains("## porq (progressive orchestration)"));
    }

    #[test]
    fn cursor_pack_writes_skill() {
        let dir = tempdir().unwrap();
        let written = integrate_cursor(dir.path()).unwrap();
        assert!(written.iter().any(|p| p.contains("SKILL.md")));
        let skill = std::fs::read_to_string(dir.path().join(".cursor/skills/porq/SKILL.md")).unwrap();
        assert_eq!(skill, SKILL_MD);
    }
}
