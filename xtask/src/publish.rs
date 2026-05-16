//! Publish crates to crates.io in dependency order with staggered delays.
//!
//! crates.io rate-limits new crate registrations to roughly one per minute.
//! Every publish is followed by a configurable delay (default: 90 s) so the
//! registry has time to index the crate before dependents are uploaded.
//!
//! Publish order (each line is one crate, published sequentially):
//!   1. agentlint-core          — no workspace deps
//!   2. agentlint-frontmatter   — depends on core
//!   3. agentlint-claude        — depends on core + frontmatter
//!   4. agentlint-cursor        — depends on core + frontmatter
//!   5. agentlint-codex         — depends on core
//!   6. agentlint-gemini        — depends on core
//!   7. agentlint-pi            — depends on core
//!   8. agentlint-opencode      — depends on core
//!   9. agentlint               — depends on all of the above

use anyhow::{Context, Result};
use std::path::Path;
use xshell::{Shell, cmd};

/// Seconds to wait between each crate publish.
/// New crates are rate-limited to ~1/min; subsequent versions of existing crates
/// are indexed much faster. 20 s is sufficient for known crates.
const STAGGER_SECS: u64 = 20;

/// Publish order — strictly sequential, each entry depends on all prior entries.
const PUBLISH_ORDER: &[&str] = &[
    "agentlint-core",
    "agentlint-frontmatter",
    "agentlint-claude",
    "agentlint-cursor",
    "agentlint-codex",
    "agentlint-gemini",
    "agentlint-pi",
    "agentlint-opencode",
    "agentlint",
];

/// `from_crate`: if `Some`, skip all crates before this one (resume after partial run).
/// Already-published crates are skipped automatically (crates.io returns 200 on re-upload
/// of an identical version, but errors on version conflict — we detect "already uploaded"
/// in stderr and treat it as success).
pub fn publish(sh: &Shell, root: &Path, from_crate: Option<&str>) -> Result<()> {
    if std::env::var("CARGO_REGISTRY_TOKEN").is_err() {
        anyhow::bail!(
            "CARGO_REGISTRY_TOKEN is not set.\n\
             Inject it via:\n\
             \n\
             CARGO_REGISTRY_TOKEN=$(op item get tok5hcvdvhows2kcgjfhrpohpa \\\n\
               --account VOKIF3CQ7VHTLN3SXXAPBWLY3E --fields token --reveal) \\\n\
               cargo xtask publish\n\
             \n\
             Or run `cargo login` once to store the token in ~/.cargo/credentials.toml"
        );
    }
    let total = PUBLISH_ORDER.len();
    let start = match from_crate {
        None => 0,
        Some(name) => PUBLISH_ORDER
            .iter()
            .position(|&k| k == name)
            .with_context(|| format!("unknown crate '{name}'; valid names: {PUBLISH_ORDER:?}"))?,
    };

    for (i, &krate) in PUBLISH_ORDER.iter().enumerate().skip(start) {
        let n = i + 1;
        let manifest = if krate == "agentlint" {
            root.join("Cargo.toml")
        } else {
            root.join("crates").join(krate).join("Cargo.toml")
        };

        eprintln!("[{n}/{total}] publishing {krate}...");
        let output = cmd!(sh, "cargo publish --manifest-path {manifest}")
            .ignore_status()
            .output()
            .with_context(|| format!("failed to run cargo publish for {krate}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            // published successfully
        } else if stderr.contains("already uploaded") || stderr.contains("already exists") {
            eprintln!("  {krate} already published — skipping");
        } else {
            anyhow::bail!("failed to publish {krate}:\n{stderr}");
        }

        if n < total {
            eprintln!("  waiting {STAGGER_SECS}s before next publish...");
            std::thread::sleep(std::time::Duration::from_secs(STAGGER_SECS));
        }
    }
    eprintln!("✓ all {total} crates published");
    Ok(())
}
