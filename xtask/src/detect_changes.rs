//! CI change detection: classify changed paths into workspace areas.
//!
//! Used by `cargo xtask detect-changes <base-ref>` to emit GitHub Actions
//! outputs that downstream jobs can use to skip work that hasn't changed.
//!
//! Output format (printed to stdout or appended to `$GITHUB_OUTPUT`):
//! ```text
//! core=true
//! frontmatter=false
//! ...
//! ```

use anyhow::{Context, Result};
use std::path::Path;
use xshell::{Shell, cmd};

// Types

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Core,
    Frontmatter,
    Xtask,
    Docs,
    Workflows,
    Plugins,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChangeSet {
    pub core: bool,
    pub frontmatter: bool,
    pub xtask: bool,
    pub docs: bool,
    pub workflows: bool,
    pub plugins: bool,
}

impl ChangeSet {
    fn set(&mut self, area: Area) {
        match area {
            Area::Core => self.core = true,
            Area::Frontmatter => self.frontmatter = true,
            Area::Xtask => self.xtask = true,
            Area::Docs => self.docs = true,
            Area::Workflows => self.workflows = true,
            Area::Plugins => self.plugins = true,
        }
    }
}

// Path classifier

/// Map a changed file path (relative to workspace root) to a workspace area.
///
/// Returns `None` for paths that don't match any tracked area (e.g. `target/`).
pub fn classify_path(path: &str) -> Option<Area> {
    if path.starts_with("crates/agentlint-core/") || path == "Cargo.toml" || path == "Cargo.lock" {
        Some(Area::Core)
    } else if path.starts_with("crates/agentlint-frontmatter/") {
        Some(Area::Frontmatter)
    } else if path.starts_with("crates/agentlint-plugins/") || path.starts_with("plugins/") {
        Some(Area::Plugins)
    } else if path.starts_with("xtask/") {
        Some(Area::Xtask)
    } else if path.starts_with("docs/") || (path.ends_with(".md") && !path.contains('/')) {
        Some(Area::Docs)
    } else if path.starts_with(".github/") {
        Some(Area::Workflows)
    } else {
        None
    }
}

// Public API

/// Run `git diff --name-only <base_ref>...HEAD` and classify changed paths.
pub fn detect_changes(root: &Path, base_ref: &str) -> Result<ChangeSet> {
    let sh = Shell::new()?;
    sh.change_dir(root);

    let range = format!("{base_ref}...HEAD");
    let output = cmd!(sh, "git diff --name-only {range}").read()?;

    let mut cs = ChangeSet::default();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(area) = classify_path(line) {
            cs.set(area);
        }
    }
    Ok(cs)
}

/// Serialise a `ChangeSet` to `key=value` output lines.
pub fn changeset_to_output_lines(cs: &ChangeSet) -> Vec<String> {
    vec![
        format!("core={}", cs.core),
        format!("frontmatter={}", cs.frontmatter),
        format!("plugins={}", cs.plugins),
        format!("xtask={}", cs.xtask),
        format!("docs={}", cs.docs),
        format!("workflows={}", cs.workflows),
    ]
}

/// Write outputs to `$GITHUB_OUTPUT` if set, otherwise print to stdout.
pub fn emit_gha_outputs(cs: &ChangeSet) -> Result<()> {
    use std::io::Write;

    let lines = changeset_to_output_lines(cs);

    if let Ok(output_path) = std::env::var("GITHUB_OUTPUT") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output_path)
            .with_context(|| format!("failed to open GITHUB_OUTPUT: {output_path}"))?;
        for line in &lines {
            writeln!(f, "{line}")?;
        }
    } else {
        for line in &lines {
            println!("{line}");
        }
    }
    Ok(())
}

/// Detects changed workspace areas and emits their GitHub Actions outputs.
pub fn run(root: &Path, base_ref: &str) -> Result<()> {
    let cs = detect_changes(root, base_ref)?;
    emit_gha_outputs(&cs)
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_core_crate() {
        assert_eq!(
            classify_path("crates/agentlint-core/src/lib.rs"),
            Some(Area::Core)
        );
    }

    #[test]
    fn classify_workspace_cargo_toml_is_core() {
        assert_eq!(classify_path("Cargo.toml"), Some(Area::Core));
        assert_eq!(classify_path("Cargo.lock"), Some(Area::Core));
    }

    #[test]
    fn classify_frontmatter() {
        assert_eq!(
            classify_path("crates/agentlint-frontmatter/src/lib.rs"),
            Some(Area::Frontmatter)
        );
    }

    #[test]
    fn classify_plugins() {
        assert_eq!(
            classify_path("crates/agentlint-plugins/src/lib.rs"),
            Some(Area::Plugins)
        );
        assert_eq!(classify_path("plugins/claude.toml"), Some(Area::Plugins));
    }

    #[test]
    fn classify_xtask() {
        assert_eq!(classify_path("xtask/src/gates.rs"), Some(Area::Xtask));
    }

    #[test]
    fn classify_docs_subdir() {
        assert_eq!(
            classify_path("docs/plans/2026-05-15-agentlint.md"),
            Some(Area::Docs)
        );
    }

    #[test]
    fn classify_root_md() {
        assert_eq!(classify_path("README.md"), Some(Area::Docs));
        assert_eq!(classify_path("CHANGELOG.md"), Some(Area::Docs));
    }

    #[test]
    fn classify_workflows() {
        assert_eq!(
            classify_path(".github/workflows/pr.yml"),
            Some(Area::Workflows)
        );
    }

    #[test]
    fn classify_unknown_returns_none() {
        assert_eq!(classify_path("src/main.rs"), None);
        assert_eq!(classify_path(".ctx/HANDOFF.yaml"), None);
        assert_eq!(classify_path("deny.toml"), None);
    }

    #[test]
    fn changeset_folds_multiple_paths() {
        let paths = [
            "crates/agentlint-core/src/lib.rs",
            "plugins/claude.toml",
            "docs/plans/2026-05-15-agentlint.md",
        ];
        let mut cs = ChangeSet::default();
        for p in &paths {
            if let Some(area) = classify_path(p) {
                cs.set(area);
            }
        }
        assert!(cs.core);
        assert!(cs.plugins);
        assert!(cs.docs);
        assert!(!cs.frontmatter);
    }

    #[test]
    fn emit_outputs_formats_correctly() {
        let cs = ChangeSet {
            core: true,
            frontmatter: false,
            plugins: true,
            xtask: false,
            docs: false,
            workflows: false,
        };
        let lines = changeset_to_output_lines(&cs);
        assert!(lines.contains(&"core=true".to_string()));
        assert!(lines.contains(&"plugins=true".to_string()));
        assert!(lines.contains(&"frontmatter=false".to_string()));
    }

    #[test]
    fn detect_changes_with_real_git() {
        use std::process::Command;
        use tempfile::TempDir;

        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();

        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .expect("git")
        };

        git(&["init", "-b", "main"]);
        git(&["config", "user.email", "test@test.com"]);
        git(&["config", "user.name", "Test"]);

        std::fs::create_dir_all(root.join("crates/agentlint-core/src")).expect("mkdir");
        std::fs::write(root.join("crates/agentlint-core/src/lib.rs"), b"// v1").expect("write");
        git(&["add", "."]);
        git(&["commit", "-m", "initial"]);

        std::fs::write(root.join("crates/agentlint-core/src/lib.rs"), b"// v2").expect("write");
        std::fs::create_dir_all(root.join("docs")).expect("mkdir");
        std::fs::write(root.join("docs/ARCHITECTURE.md"), b"# arch").expect("write");
        git(&["add", "."]);
        git(&["commit", "-m", "update"]);

        let cs = detect_changes(root, "HEAD^").expect("detect_changes");
        assert!(cs.core, "core should be true");
        assert!(cs.docs, "docs should be true");
        assert!(!cs.frontmatter, "frontmatter should be false");
    }
}
