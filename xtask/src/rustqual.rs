//! rustqual integration — local quality gate.
//!
//! Usage: `cargo xtask pre-push`

use anyhow::{Context, Result};
use xshell::{Shell, cmd};

/// Run rustqual regression check against the committed baseline.
pub fn pre_push(sh: &Shell) -> Result<()> {
    cmd!(
        sh,
        "rustqual --compare .rustqual-baseline.json --fail-on-regression"
    )
    .run()
    .context("rustqual regression check failed")?;
    eprintln!("rustqual gate passed");
    Ok(())
}
