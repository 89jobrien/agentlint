//! Quality gates: ci, fix, pre-commit, pre-push.

use anyhow::{Context, Result};
use xshell::{Shell, cmd};

/// CI gate: fmt-check + clippy -D warnings + nextest.
pub fn ci(sh: &Shell) -> Result<()> {
    cmd!(sh, "cargo fmt --all --check")
        .run()
        .context("cargo fmt --check failed")?;
    cmd!(sh, "cargo clippy --workspace -- -D warnings")
        .run()
        .context("cargo clippy failed")?;
    cmd!(sh, "cargo nextest run --workspace")
        .run()
        .context("cargo nextest run failed")?;
    eprintln!("ci gate passed");
    Ok(())
}

/// Fix gate: fmt + clippy --fix (mutates files).
pub fn fix(sh: &Shell) -> Result<()> {
    cmd!(sh, "cargo fmt --all")
        .run()
        .context("cargo fmt failed")?;
    cmd!(
        sh,
        "cargo clippy --workspace --fix --allow-dirty --allow-staged"
    )
    .run()
    .context("cargo clippy --fix failed")?;
    eprintln!("fix gate passed");
    Ok(())
}

/// Pre-commit gate: fmt-check + clippy (validation only, no file mutations).
pub fn pre_commit(sh: &Shell) -> Result<()> {
    if staged_rust_files(sh)? {
        cmd!(sh, "cargo fmt --all --check")
            .run()
            .context("fmt-check failed")?;
        cmd!(sh, "cargo clippy --workspace -- -D warnings")
            .run()
            .context("clippy failed")?;
    }
    if staged_workflow_files(sh)? {
        cmd!(
            sh,
            "actionlint -config-file .github/actionlint.yaml .github/workflows/ .github/actions/"
        )
        .run()
        .context("actionlint failed")?;
    }
    eprintln!("pre-commit gate passed");
    Ok(())
}

fn staged_rust_files(sh: &Shell) -> Result<bool> {
    let out = cmd!(sh, "git diff --cached --name-only").read()?;
    Ok(out.lines().any(|l| l.ends_with(".rs")))
}

fn staged_workflow_files(sh: &Shell) -> Result<bool> {
    let out = cmd!(sh, "git diff --cached --name-only").read()?;
    Ok(out.lines().any(|l| l.starts_with(".github/")))
}
