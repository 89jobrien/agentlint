//! Release workflow: bump → commit → tag → push → GH release.
//!
//! Usage: `cargo xtask release [patch|minor|major]`

use anyhow::{Context, Result};
use std::path::Path;
use xshell::{Shell, cmd};

use crate::utils::parse_workspace_version;

pub fn release(sh: &Shell, root: &Path, level: &str) -> Result<()> {
    // 1. Bump workspace version.
    crate::bump::bump(root, level)?;

    // 2. Read the new version.
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let version = parse_workspace_version(&manifest)
        .ok_or_else(|| anyhow::anyhow!("could not read version after bump"))?;
    let tag = format!("v{version}");

    // 3. Commit, tag, push.
    cmd!(sh, "git add Cargo.toml Cargo.lock")
        .run()
        .context("git add failed")?;
    cmd!(sh, "git commit -m {tag}")
        .run()
        .context("git commit failed")?;
    cmd!(sh, "git tag {tag}").run().context("git tag failed")?;
    cmd!(sh, "git push origin HEAD {tag}")
        .run()
        .context("git push failed")?;

    // 4. Create GitHub release (triggers publish workflow).
    cmd!(sh, "gh release create {tag} --generate-notes --title {tag}")
        .run()
        .context("gh release create failed")?;

    eprintln!("released {tag}");
    Ok(())
}
