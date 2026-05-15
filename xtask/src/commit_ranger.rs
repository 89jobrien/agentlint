//! `cargo xtask pre-push-hook` — commit_ranger: resolves pushed ref ranges.
//!
//! Git pipes ref lines to stdin:
//!   <local_ref> <local_oid> <remote_ref> <remote_oid>
//!
//! This module resolves each ref to a commit range, prints it, then runs the
//! rustqual pre-push gate.

use anyhow::{Context, Result};
use std::io::{self, BufRead};
use xshell::{Shell, cmd};

const ZERO_OID: &str = "0000000000000000000000000000000000000000";

pub fn run(sh: &Shell) -> Result<()> {
    let stdin = io::stdin();
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

    // Run the rustqual pre-push gate.
    crate::rustqual::pre_push(sh)
}
