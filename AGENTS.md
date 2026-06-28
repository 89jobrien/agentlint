# Agentlint — Agent Operating Guide

This file instructs AI coding agents (Codex, Claude, Copilot, or any shell-capable
LLM agent) how to work effectively in the agentlint codebase.

## Identity

You are working on **agentlint**, a linter for AI coding agent harness files
(Claude Code, Cursor, Codex, Gemini, OpenCode, Pi). The codebase is a Rust
workspace (edition 2024) with a thin CLI binary and four validation crates.

## Primary Toolkit: `cargo` + `mise`

All development workflows use cargo directly. The `mise` task runner provides
simplified shortcuts for common operations.

### Build & Quality

| cargo                                  | mise             | Purpose                           |
| -------------------------------------- | ---------------- | --------------------------------- |
| `cargo build --release`                | `mise run build` | Build release binary              |
| `cargo check --workspace`              | —                | Fast type-check all crates        |
| `cargo fmt --all`                      | —                | Format all Rust code              |
| `cargo fmt --all --check`              | —                | Check formatting (no modify)      |
| `cargo clippy --workspace -- -D warn`  | —                | Run clippy with warnings as error |
| `cargo nextest run --workspace`        | —                | Run all tests                     |
| `cargo nextest run -E 'test(pattern)'` | —                | Run tests matching pattern        |
| `cargo doc --workspace --no-deps`      | —                | Build workspace docs              |

### Project Setup

| mise               | Purpose                  |
| ------------------ | ------------------------ |
| `mise run install` | Build and install binary |

## Workspace Layout

```
agentlint/
├── src/main.rs              # CLI entry point (thin wrapper)
├── crates/
│   ├── agentlint-core/      # Core runner, validator trait, diagnostics,
│   │                        #   output formatters
│   ├── agentlint-frontmatter/# YAML frontmatter parser (nom-based)
│   ├── agentlint-docs/      # Doc validator: frontmatter + schema registry
│   └── agentlint-plugins/   # Embedded TOML plugin definitions
├── xtask/                   # Custom build scripts (not in use)
├── plugins/                 # Per-agent TOML plugin files
├── .cargo/config.toml       # Cargo config (profile overrides)
├── deny.toml                # Dependency audit rules
├── rustqual.toml            # Code quality metrics config
└── .mise.toml               # Task definitions
```

## Code Conventions

### Rust (Edition 2024)

- **Line width**: 100 characters
- **Linting**: `cargo clippy --workspace -- -D warnings`
- **Error handling**: Return `Vec<Diagnostic>` in validators; accumulate errors
  rather than fail-fast
- **Testing**: Unit tests in `mod tests {}`, pattern-based discovery via
  `cargo nextest run`
- **Naming**: PascalCase structs/enums, snake_case functions/variables

### Core Abstractions

- **Diagnostic**: Single validation output unit with `path`, `line`, `col`,
  `message`, and `severity` (Error|Warning)
- **Validator trait**: `fn validate(path: &Path, src: &str) -> Vec<Diagnostic>`
  — each per-agent crate implements this
- **Runner**: Walks filesystem, pattern-matches files to validators, collects
  all diagnostics
- **Output**: GNU format (path:line:col: error: msg) by default, JSON via
  `--format json`

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

- Types: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`
- Example: `fix(claude): validate required frontmatter fields`

## Key Dependencies

| Crate     | Purpose                                 |
| --------- | --------------------------------------- |
| `nom`     | Frontmatter parser (Claude, Cursor)     |
| `clap`    | CLI arg parsing (derive feature)        |
| `serde`   | Config serialization (YAML, JSON, TOML) |
| `walkdir` | Recursive file discovery                |

## Workflow: Add a New Validator

1. Create a crate: `cargo new --lib crates/agentlint-<agent-name>`
2. Implement `Validator` from `agentlint-core` — return `Vec<Diagnostic>`
3. Register patterns in crate module: which files it owns
4. Add to workspace `Cargo.toml` members list
5. Import and register in `agentlint-plugins` or binary runner
6. Test: `cargo nextest run -p agentlint-<agent-name>`
7. Commit: `git commit -m "feat(plugins): add <agent-name> validator"`

## Workflow: Debug a Validation Error

1. Run with verbose output: `agentlint --format gnu <file>`
2. Check which validator matched: look at patterns in crates/
3. Inspect the validator source: typically a frontmatter or JSON schema check
4. Add a test case: `cargo nextest run -E 'test(your_test_name)'`
5. Fix the validator and re-test

## Exit Codes

| Code | Meaning              |
| ---- | -------------------- |
| `0`  | All files valid      |
| `1`  | One or more errors   |
| `2`  | Internal error (I/O) |

## Before Committing

Run the standard validation suite:

```bash
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo nextest run --workspace
```

Or use a pre-commit hook to automate this. See workspace git hooks.
