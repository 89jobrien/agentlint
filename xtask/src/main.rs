//! xtask — workspace automation for agentlint.
//!
//! | Command      | Description                                               |
//! |------------- |-----------------------------------------------------------|
//! | ci           | fmt-check + clippy -D warnings + nextest (CI gate)        |
//! | fix          | fmt + clippy --fix (mutates files)                        |
//! | pre-commit   | fmt-check + clippy (validation only, fast)                |
//! | pre-push     | rustqual regression check against committed baseline       |
//! | publish      | publish crates to crates.io in dependency order (staggered)|
//! | release      | bump version, commit, tag, push, create GH release        |
//! | detect-changes | classify changed paths, emit GHA outputs               |
//! | ci-watch     | watch latest GHA run with job-level detail                |
//! | land         | fetch + rebase onto main + CI gate + push (merge queue)   |

use anyhow::{Result, bail};
use std::env;
use xshell::Shell;

mod bump;
mod ci_watch;
mod detect_changes;
mod gates;
mod land;
mod publish;
mod release;
mod rustqual;
mod utils;

fn main() -> Result<()> {
    let task = env::args().nth(1);
    let sh = Shell::new()?;

    let root = sh.current_dir();
    let root = root
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .unwrap_or(&root)
        .to_path_buf();
    sh.change_dir(&root);

    match task.as_deref() {
        Some("ci") => gates::ci(&sh),
        Some("fix") => gates::fix(&sh),
        Some("pre-commit") => gates::pre_commit(&sh),
        Some("pre-push") => rustqual::pre_push(&sh),
        Some("publish") => {
            let from = {
                let args: Vec<String> = env::args().collect();
                args.windows(2)
                    .find(|w| w[0] == "--from")
                    .map(|w| w[1].clone())
            };
            publish::publish(&sh, &root, from.as_deref())
        }
        Some("release") => {
            let level = env::args().nth(2).unwrap_or_else(|| "patch".to_string());
            release::release(&sh, &root, &level)
        }
        Some("bump") => {
            let level = env::args().nth(2).unwrap_or_else(|| "patch".to_string());
            bump::bump(&root, &level)
        }
        Some("detect-changes") => {
            let base = env::args()
                .nth(2)
                .unwrap_or_else(|| "origin/main".to_string());
            detect_changes::run(&root, &base)
        }
        Some("land") => land::land(&sh),
        Some("ci-watch") => {
            let branch = {
                let args: Vec<String> = env::args().collect();
                args.windows(2)
                    .find(|w| w[0] == "--branch")
                    .map(|w| w[1].clone())
            };
            ci_watch::ci_watch(&sh, branch.as_deref())
        }
        Some(other) => bail!("unknown task: {other}"),
        None => {
            eprintln!("Available tasks:");
            eprintln!("  ci              fmt-check + clippy + nextest (CI gate)");
            eprintln!("  fix             fmt + clippy --fix (mutates files)");
            eprintln!("  pre-commit      fmt-check + clippy (validation only)");
            eprintln!("  pre-push        rustqual regression check vs baseline");
            eprintln!(
                "  publish [--from <crate>]  publish crates to crates.io (staggered, 90s gaps)"
            );
            eprintln!("  detect-changes [<base-ref>]  classify changed paths, emit GHA outputs");
            eprintln!("  release [patch|minor|major]  bump, tag, push, create GH release");
            eprintln!("  bump [patch|minor|major]     bump workspace version only");
            eprintln!("  ci-watch [--branch <branch>] watch latest GHA run");
            eprintln!("  land                         fetch + rebase + CI gate + push");
            Ok(())
        }
    }
}
