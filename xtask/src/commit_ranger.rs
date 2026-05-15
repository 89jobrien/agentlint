//! commit_ranger — resolves pushed ref ranges for the pre-push hook.
//!
//! Invoked via `cargo xtask pre-push-hook`.
//!
//! Git pipes ref lines to stdin:
//!   <local_ref> <local_oid> <remote_ref> <remote_oid>
//!
//! This module resolves each ref to a commit range, prints it, then runs the
//! rustqual pre-push gate.

use anyhow::{Context, Result};
use std::io::{self, BufRead};
use xshell::{Shell, cmd};

// 40-char SHA-1 zero sentinel. Git uses this to signal non-existent refs.
// Note: SHA-256 repos use a 64-char zero OID — not supported here.
const ZERO_OID: &str = "0000000000000000000000000000000000000000";

pub fn run(sh: &Shell) -> Result<()> {
    let stdin = io::stdin();
    let mut has_refs = false;

    for line in stdin.lock().lines() {
        let line = line.context("failed to read pre-push stdin")?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 4 {
            continue;
        }
        let (local_oid, remote_oid) = (parts[1], parts[3]);

        // Branch deletion — nothing to inspect.
        if local_oid == ZERO_OID {
            continue;
        }

        has_refs = true;

        let base = if remote_oid == ZERO_OID {
            // New branch — find divergence from main, fall back to initial commit.
            cmd!(sh, "git merge-base {local_oid} main")
                .read()
                .or_else(|_| cmd!(sh, "git rev-list --max-parents=0 HEAD").read())
                .context("failed to resolve merge-base")?
        } else {
            remote_oid.to_string()
        };

        println!("{base}..{local_oid}");
    }

    // Skip the gate if there are no new commits (tag-only or deletion-only push).
    if !has_refs {
        return Ok(());
    }

    // Run the rustqual pre-push gate.
    crate::rustqual::pre_push(sh)
}
