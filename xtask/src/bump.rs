//! Bump the workspace version in the root Cargo.toml.
//!
//! Usage: `cargo xtask bump [patch|minor|major]`

use anyhow::{Result, bail};
use std::{fs, path::Path};

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

pub fn parse_workspace_version(content: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let t = line.trim();
        if t == "[workspace.package]" {
            in_section = true;
            continue;
        }
        if in_section {
            if t.starts_with('[') {
                break;
            }
            if let Some(v) = t
                .strip_prefix("version = \"")
                .and_then(|v| v.strip_suffix('"'))
            {
                return Some(v.to_string());
            }
        }
    }
    None
}

const SEMVER_PARTS: usize = 3;

fn parse_semver(v: &str) -> Result<(u64, u64, u64)> {
    let p: Vec<&str> = v.split('.').collect();
    if p.len() != SEMVER_PARTS {
        bail!("version {v:?} is not semver");
    }
    Ok((p[0].parse()?, p[1].parse()?, p[2].parse()?))
}
