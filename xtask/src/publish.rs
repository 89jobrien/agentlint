//! Publish crates to crates.io in dependency order.
//!
//! Order:
//!   1. agentlint-core
//!   2. per-agent crates (all depend only on core)
//!   3. agentlint (root binary)

use anyhow::{Context, Result};
use std::path::Path;
use xshell::{Shell, cmd};

/// Crates published in order. Each inner `&[&str]` is a parallel-safe wave.
const PUBLISH_WAVES: &[&[&str]] = &[
    &["agentlint-core"],
    &[
        "agentlint-claude",
        "agentlint-cursor",
        "agentlint-codex",
        "agentlint-opencode",
        "agentlint-gemini",
        "agentlint-pi",
    ],
    &["agentlint"],
];

pub fn publish(sh: &Shell, root: &Path) -> Result<()> {
    for (i, wave) in PUBLISH_WAVES.iter().enumerate() {
        eprintln!("── wave {} ──", i + 1);
        for &krate in *wave {
            let manifest = if krate == "agentlint" {
                root.join("Cargo.toml")
            } else {
                root.join("crates").join(krate).join("Cargo.toml")
            };

            eprintln!("publishing {krate}...");
            cmd!(sh, "cargo publish --manifest-path {manifest} --no-verify")
                .run()
                .with_context(|| format!("failed to publish {krate}"))?;

            // crates.io propagation delay between waves.
            if i < PUBLISH_WAVES.len() - 1 {
                eprintln!("waiting 20s for crates.io propagation...");
                std::thread::sleep(std::time::Duration::from_secs(20));
            }
        }
    }
    eprintln!("all crates published");
    Ok(())
}
