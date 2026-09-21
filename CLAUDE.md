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

Cargo workspace with a thin binary entry point and **only 4 crates**. Agent support is split
across two very different mechanisms — this split is the most important fact about the codebase:

- **Rust behavioral modules** (compiled in): Claude Code, Cursor, and Looprs get hand-written
  validators as Rust modules under `agentlint-core/src/behavioral/`.
- **Declarative TOML plugins**: Codex, OpenCode, Gemini, Pi, and Maestro-docs are _not_ Rust code
  at all — they're TOML rule files in the top-level `plugins/` directory, interpreted by the
  declarative plugin engine in `agentlint-plugins`.

```text
agentlint/
  src/main.rs                     # thin CLI wrapper — arg parsing, calls core runner
  crates/
    agentlint-core/                # Diagnostic type, Validator trait, discovery, formatters,
                                   #   runner, declarative plugin engine, and:
      src/behavioral/
        claude/                    # Claude Code: agents.rs, skills.rs, commands.rs, hooks.rs,
                                   #   settings.rs, mcp.rs, meta.rs
        cursor/                    # Cursor: .cursor/rules/**/*.mdc|.md, .cursorrules
        looprs/                    # Looprs: commands, hooks, skills YAML validation
    agentlint-frontmatter/         # Shared YAML frontmatter parser (nom-based)
    agentlint-docs/                # Docs: frontmatter schema validation, --infer-schema,
                                   #   --emit-schema, SchemaRegistry
    agentlint-plugins/             # Declarative TOML plugin engine (embeds/loads plugins/*.toml)
  plugins/                         # TOML plugin definitions, one per declarative agent:
                                   #   agentlint.codex.toml, .opencode.toml, .gemini.toml,
                                   #   .pi.toml, .maestro-docs.toml, plus .claude.toml and
                                   #   .cursor.toml (declarative supplements to the Rust modules)
```

### Core abstractions (`agentlint-core`)

- `Diagnostic { path, line, col, message, severity }` — the single output unit; severity is
  `Error` or `Warning`
- `Validator` trait: `fn validate(path: &Path, src: &str) -> Vec<Diagnostic>` — each behavioral
  module and the plugin engine implements this; validators accumulate all errors rather than
  fail-fast
- Runner: walks cwd (or explicit paths), pattern-matches files to the correct validator (Rust
  module or TOML plugin), collects diagnostics
- Output: GNU format (`path:line:col: error: msg`) or JSON via `--format json`

### Frontmatter parser (`agentlint-frontmatter`)

Extracted into its own crate. Used by Claude Code, Cursor, and Docs validators.
Grammar: `"---" newline field* "---" newline body`. Produces
`Vec<Field { key, value, line }>`. Includes a `FrontmatterValidator` builder for
declarative required-field rules. Validation is a separate layer on top of the parse
output so line numbers in diagnostics are accurate.

### Claude Code behavioral sub-modules

`agentlint-core/src/behavioral/claude/` is split into seven sub-modules: `agents`, `skills`,
`commands`, `hooks`, `settings`, `mcp`, `meta`. Agents/skills/commands share the nom frontmatter
parser and require `name` and `description` fields. Hooks check shebang + execute bit. Settings
uses `serde_json` and validates known top-level keys. `mcp` validates MCP server config; `meta`
covers cross-cutting/meta-level checks.

### Declarative plugins (`agentlint-plugins` + `plugins/`)

Codex, OpenCode, Gemini, Pi, and Maestro-docs have no Rust validator — their rules live entirely
as TOML in `plugins/*.toml` and are interpreted at runtime by the engine in `agentlint-plugins`.
Adding support for a new simple agent format means writing a TOML file, not a crate.

## Key dependencies

| Crate        | Purpose                                  |
| ------------ | ---------------------------------------- |
| `nom`        | Frontmatter parser (Claude Code, Cursor) |
| `clap`       | CLI arg parsing (derive feature)         |
| `serde_json` | JSON parsing for settings / opencode     |
| `serde`      | Serialization for schemas and plugins    |
| `toml`       | Declarative plugin definitions           |

## Exit codes

| Code | Meaning                       |
| ---- | ----------------------------- |
| `0`  | All files valid               |
| `1`  | One or more validation errors |
| `2`  | Internal error (I/O, etc.)    |
