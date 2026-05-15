//! Shared utilities for xtask commands.

use anyhow::{Result, bail};

const SEMVER_PARTS: usize = 3;

/// Extract the version string from a `[workspace.package]` section in a
/// Cargo.toml file's content.
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

/// Parse a `"major.minor.patch"` version string into its three components.
pub fn parse_semver(v: &str) -> Result<(u64, u64, u64)> {
    let p: Vec<&str> = v.split('.').collect();
    if p.len() != SEMVER_PARTS {
        bail!("version {v:?} is not semver");
    }
    Ok((p[0].parse()?, p[1].parse()?, p[2].parse()?))
}
