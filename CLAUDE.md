# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this
repository.

## Commands

```bash
cargo check --workspace
cargo nextest run --workspace
cargo clippy --workspace -- -D warnings

# Run a single test
cargo nextest run --workspace -E 'test(test_name)'
```

## Architecture

Cargo workspace with a thin binary entry point and one library crate per agent platform:

```
agentlint/
  src/main.rs              # thin CLI wrapper — arg parsing, calls core runner
  crates/
    agentlint-core/        # Diagnostic type, Validator trait, file discovery, output formatters, runner
    agentlint-claude/      # Claude Code: agents, skills, commands, hooks, settings
    agentlint-cursor/      # Cursor: .cursor/rules/**/*.mdc|.md, .cursorrules
    agentlint-codex/       # Codex: AGENTS.md
    agentlint-opencode/    # OpenCode: AGENTS.md, opencode.json
    agentlint-gemini/      # Gemini: GEMINI.md
    agentlint-pi/          # Pi: AGENTS.md, SYSTEM.md
```

### Core abstractions (`agentlint-core`)

- `Diagnostic { path, line, col, message, severity }` — the single output unit; severity is
  `Error` or `Warning`
- `Validator` trait: `fn validate(path: &Path, src: &str) -> Vec<Diagnostic>` — each per-agent
  crate implements this; validators accumulate all errors rather than fail-fast
- Runner: walks cwd (or explicit paths), pattern-matches files to the correct validator, collects
  diagnostics
- Output: GNU format (`path:line:col: error: msg`) or JSON via `--format json`

### Frontmatter parser

Claude Code and Cursor files use a `nom`-based frontmatter parser shared across those crates.
Grammar: `"---" newline field* "---" newline body`. Produces
`Vec<Field { key, value, line }>`. Validation is a separate layer on top of the parse output so
line numbers in diagnostics are accurate.

### Per-agent crate structure

Each per-agent crate:

1. Declares which file patterns it owns (used by core for dispatch)
2. Implements `Validator` from `agentlint-core`
3. Uses `nom` for frontmatter (Claude, Cursor) or `serde_json` / line-based checks elsewhere

### Claude Code validator sub-modules

`agentlint-claude` is split into five sub-modules: `agents`, `skills`, `commands`, `hooks`,
`settings`. Agents/skills/commands share the nom frontmatter parser and both require `name` and
`description` fields. Hooks check shebang + execute bit. Settings uses `serde_json` and validates
known top-level keys.

## Key dependencies

| Crate        | Purpose                                  |
| ------------ | ---------------------------------------- |
| `nom`        | Frontmatter parser (Claude Code, Cursor) |
| `clap`       | CLI arg parsing (derive feature)         |
| `serde_json` | JSON parsing for settings / opencode     |

## Exit codes

| Code | Meaning                       |
| ---- | ----------------------------- |
| `0`  | All files valid               |
| `1`  | One or more validation errors |
| `2`  | Internal error (I/O, etc.)    |
