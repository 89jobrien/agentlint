//! `cargo xtask land` — local merge-queue emulation.
//!
//! Steps:
//!   1. Refuse to run on `main` directly.
//!   2. `git fetch origin main`
//!   3. Rebase current branch onto `origin/main`.
//!   4. Run the full CI gate (fmt-check + clippy + nextest).
//!   5. `git push` (fast-forward only).

use anyhow::{Context, Result, bail};
use xshell::{Shell, cmd};

pub fn land(sh: &Shell) -> Result<()> {
    // 1. Guard: refuse to land from main itself.
    let branch = cmd!(sh, "git branch --show-current")
        .read()
        .context("failed to determine current branch")?;
    let branch = branch.trim();
    if branch == "main" {
        bail!("land must be run from a feature branch, not main");
    }

    // 2. Fetch latest main.
    eprintln!("→ fetching origin/main");
    cmd!(sh, "git fetch origin main")
        .run()
        .context("git fetch failed")?;

    // 3. Rebase onto origin/main.
    eprintln!("→ rebasing {branch} onto origin/main");
    cmd!(sh, "git rebase origin/main")
        .run()
        .context("git rebase failed — resolve conflicts then re-run `cargo xtask land`")?;

    // 4. Full CI gate.
    eprintln!("→ running CI gate");
    crate::gates::ci(sh).context("CI gate failed — fix issues before landing")?;

    // 5. Push.
    eprintln!("→ pushing {branch}");
    cmd!(sh, "git push").run().context("git push failed")?;

    eprintln!("✓ landed {branch} onto main");
    Ok(())
}
