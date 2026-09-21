# agentlint xtask

Workspace automation for agentlint. The `xtask` package is a private binary crate and is not
published. Run it from anywhere beneath the workspace root with:

```console
cargo xtask <command>
```

The repository's Cargo alias resolves that invocation to `cargo run -p xtask --`.

## Commands

| Command                      | Behavior                                                                            |
| ---------------------------- | ----------------------------------------------------------------------------------- |
| `ci`                         | Runs `cargo fmt --all --check`, clippy with warnings denied, and workspace nextest. |
| `fix`                        | Runs formatting and `cargo clippy --fix`; this command modifies source files.       |
| `pre-commit`                 | Checks staged Rust with fmt/clippy and staged GitHub files with actionlint.         |
| `pre-push`                   | Runs rustqual against `.rustqual-baseline.json` and fails on regression.            |
| `detect-changes [BASE]`      | Classifies changes from `BASE...HEAD`; defaults to `origin/main`.                   |
| `bump [LEVEL]`               | Updates only the workspace version; `LEVEL` defaults to `patch`.                    |
| `publish [--from CRATE]`     | Publishes packages in order, optionally resuming at a crate.                        |
| `release [LEVEL]`            | Bumps, commits, tags, pushes, and creates a GitHub release.                         |
| `ci-watch [--branch BRANCH]` | Watches the latest GitHub Actions run and prints per-job status.                    |
| `land`                       | Rebases a feature branch onto `origin/main`, runs `ci`, then pushes.                |
| `pre-push-hook`              | Resolves pushed commit ranges from Git stdin, then runs `pre-push`.                 |

Use `cargo xtask` with no command to print the built-in command summary. Unknown commands fail.

## Quality gates

`ci` is the complete local validation gate:

```console
cargo xtask ci
```

`pre-commit` is intentionally index-aware. It reads `git diff --cached --name-only`; Rust checks
run only when a staged `.rs` file exists, and actionlint runs only when a staged path begins with
`.github/`. It does not run tests. `fix` is the only quality command that edits code.

`pre-push` invokes:

```console
rustqual --compare .rustqual-baseline.json --fail-on-regression
```

The `pre-push-hook` command reads Git's four-field pre-push records from stdin. New branches are
compared with their merge base against `main`; existing branches use the remote object ID. Branch
deletions and pushes with no new commit refs skip rustqual. The range resolver currently recognizes
Git's 40-character SHA-1 zero sentinel, not a 64-character SHA-256 sentinel.

## Change detection

`detect-changes` emits boolean GitHub Actions outputs for `core`, `frontmatter`, `plugins`, `xtask`,
`docs`, and `workflows`. If `GITHUB_OUTPUT` is set, lines are appended there; otherwise they are
printed to stdout.

```console
cargo xtask detect-changes origin/main
```

Classification is path-based. The root `Cargo.toml` and `Cargo.lock` count as core; plugin crate or
`plugins/` changes count as plugins; root Markdown counts as docs. Unclassified paths are ignored.

## Versioning, publishing, and release

`bump` reads `workspace.package.version` from the root manifest and accepts `patch`, `minor`, or
`major`. It writes `Cargo.toml` but does not update `Cargo.lock` or create a commit.

`publish` requires `CARGO_REGISTRY_TOKEN` and publishes sequentially:

1. `agentlint-core`
2. `agentlint-frontmatter`
3. `agentlint-docs`
4. `agentlint-plugins`
5. `agentlint`

The implementation waits 20 seconds between packages so crates.io can index dependencies. Existing
versions reported as already uploaded or already present are treated as successful skips.

`release` is intentionally stateful and should only run from a release-ready worktree. It calls
`bump`, stages `Cargo.toml` and `Cargo.lock`, commits with the `v<version>` tag as the message,
creates that tag, pushes `HEAD` and the tag, then runs `gh release create --generate-notes`.

`land` is similarly stateful. It refuses `main` and an in-progress rebase, fetches `origin/main`,
rebases the current branch, runs the full CI gate, and performs a normal `git push`.

## Dependencies and tests

The crate uses `xshell` for process execution, `anyhow` for contextual errors, `serde_json` for
GitHub CLI responses, and `chrono` for CI timing. Its tests cover change classification and output,
plus a temporary-repository change-detection integration case.

```console
cargo nextest run -p xtask
cargo clippy -p xtask -- -D warnings
cargo fmt --all --check
```
