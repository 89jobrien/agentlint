//! Bump the workspace version in the root Cargo.toml.
//!
//! Usage: `cargo xtask bump [patch|minor|major]`

use anyhow::{Result, bail};
use std::{fs, path::Path};

use crate::utils::{parse_semver, parse_workspace_version};

pub fn bump(root: &Path, level: &str) -> Result<()> {
    let manifest = root.join("Cargo.toml");
    let content = fs::read_to_string(&manifest)?;

    let current = parse_workspace_version(&content)
        .ok_or_else(|| anyhow::anyhow!("could not find [workspace.package] version"))?;

    let (major, minor, patch) = parse_semver(&current)?;
    let next = match level {
        "patch" => format!("{major}.{minor}.{}", patch + 1),
        "minor" => format!("{major}.{}.0", minor + 1),
        "major" => format!("{}.0.0", major + 1),
        other => bail!("unknown level: {other} (expected patch, minor, major)"),
    };

    let updated = content.replace(
        &format!("version = \"{current}\""),
        &format!("version = \"{next}\""),
    );
    if updated == content {
        bail!("version string not found in Cargo.toml");
    }

    fs::write(&manifest, updated)?;
    println!("version bumped {current} → {next}");
    Ok(())
}
