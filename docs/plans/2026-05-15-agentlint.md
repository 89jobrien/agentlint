# agentlint — Design Document

**Date**: 2026-05-15
**Status**: approved
**Author**: Joseph O'Brien

---

## Goal

Build `agentlint` — a Rust CLI tool that validates agent harness files used by AI coding
agents (Claude Code, Codex, Cursor, Gemini, OpenCode, Pi). Validates agents, skills,
commands, hooks, and settings. Runs in CI, exits non-zero on invalid files, emits
GNU-style or JSON diagnostics.

---

## Architecture

### Workspace layout

```
agentlint/                        # ~/dev/agentlint
  Cargo.toml                      # workspace manifest
  crates/
    agentlint-core/               # discovery, runner, output formatting
    agentlint-claude/             # Claude Code agent validator
    agentlint-cursor/             # Cursor rules validator
    agentlint-codex/              # Codex AGENTS.md validator
    agentlint-opencode/           # OpenCode AGENTS.md / opencode.json validator
    agentlint-gemini/             # Gemini GEMINI.md validator
    agentlint-pi/                 # Pi AGENTS.md / SYSTEM.md validator
  src/
    main.rs                       # agentlint binary (thin CLI wrapper)
```

### Crate responsibilities

**`agentlint-core`**

- `Diagnostic` type: file path, line, column, message, severity (`Error` | `Warning`)
- `Validator` trait: `fn validate(path: &Path, src: &str) -> Vec<Diagnostic>`
- File discovery: walks cwd (or explicit paths) matching known harness file patterns
- Output formatters: GNU (`path:line:col: error: msg`) and JSON
- CLI arg parsing (`clap`): `[paths...]`, `--format gnu|json`, `--exit-zero`
- Runner: dispatches discovered files to the correct per-agent validator

**Per-agent crates** (`agentlint-claude`, `agentlint-cursor`, etc.)

- Each implements `Validator` from `agentlint-core`
- File pattern(s) it owns (used by core for dispatch)
- nom-based frontmatter parser where applicable; simple checks elsewhere

---

## Per-agent schemas (sourced from official docs)

### Claude Code — `agentlint-claude`

`agentlint-claude` validates five artifact types, each in its own sub-module.

#### Agents — `claude::agents`

**Files**: `.claude/agents/**/*.md`
**Format**: YAML frontmatter + markdown body
**Parser**: nom

Required fields (hard error if missing or empty):

- `name` — non-empty string, lowercase letters and hyphens only
- `description` — non-empty string

Optional fields (parsed, shape not validated in v1):
`tools`, `disallowedTools`, `model`, `permissionMode`, `maxTurns`, `skills`,
`mcpServers`, `hooks`, `memory`, `background`, `effort`, `isolation`, `color`,
`initialPrompt`

#### Skills — `claude::skills`

**Files**: `.claude/skills/**/*.md`
**Format**: YAML frontmatter + markdown body
**Parser**: nom (shared frontmatter parser with agents)

Required fields:

- `name` — non-empty string
- `description` — non-empty string

Optional fields (not validated in v1): `argument-hint`

#### Commands — `claude::commands`

**Files**: `.claude/commands/**/*.md`
**Format**: YAML frontmatter + markdown body
**Parser**: nom (shared frontmatter parser)

Required fields:

- `name` — non-empty string
- `description` — non-empty string

Optional fields (not validated in v1): `disable-model-invocation`

#### Hooks — `claude::hooks`

**Files**: `.claude/hooks/*` (no extension, executable scripts)
**Format**: executable script (bash, nu, python, etc.)
**Parser**: none — line-based check

Validations:

- File has a shebang line (`#!`) on line 1
- File permission has execute bit set (Unix mode check)

Hard error: missing shebang or non-executable.

#### Settings — `claude::settings`

**Files**: `.claude/settings.json`, `.claude/settings.local.json`
**Format**: JSON
**Parser**: `serde_json`

Validations:

- Valid JSON
- Top-level keys are known (`permissions`, `env`, `hooks`, `mcpServers`, `model`,
  `apiKeyHelper`, `includeCoAuthoredBy`, `enabledMcpjsonServers`)
- `permissions.allow` and `permissions.deny` are arrays of strings if present
- `hooks` values are arrays of hook objects with `command` string if present

Hard error: invalid JSON or unknown top-level key.

### Cursor — `agentlint-cursor`

**Files**: `.cursor/rules/**/*.mdc`, `.cursor/rules/**/*.md`, `.cursorrules`
**Format**: Optional YAML frontmatter + markdown body
**Parser**: nom (frontmatter present detection, then field extraction)

No required fields. Validates when frontmatter is present:

- `description` — string if present
- `globs` — string or list if present
- `alwaysApply` — boolean if present

Hard error: malformed frontmatter (unclosed `---` fence, invalid YAML structure).

### Codex — `agentlint-codex`

**Files**: `AGENTS.md`
**Format**: Plain markdown, no frontmatter
**Parser**: none (presence + content check)

Hard error: file is empty or whitespace-only.

### OpenCode — `agentlint-opencode`

**Files**: `AGENTS.md`, `opencode.json`
**Format**: Plain markdown / JSON
**Parser**: serde_json for `opencode.json`; presence check for `AGENTS.md`

Hard error: `opencode.json` is not valid JSON.

### Gemini — `agentlint-gemini`

**Files**: `GEMINI.md`
**Format**: Plain markdown, no frontmatter
**Parser**: none (presence + content check)

Hard error: file is empty or whitespace-only.

### Pi — `agentlint-pi`

**Files**: `AGENTS.md`, `SYSTEM.md` (under `~/.pi/agent/` or project root)
**Format**: Plain markdown, no frontmatter
**Parser**: none (presence + content check)

Hard error: file is empty or whitespace-only.

---

## nom parser design (Claude Code + Cursor)

```
frontmatter  = "---" newline field* "---" newline body
field        = key ":" ws value newline
             | key ":" newline indent value newline  (multiline)
key          = [a-zA-Z][a-zA-Z0-9_-]*
value        = [^\n]+
body         = .*   (remainder, not validated)
```

Produces `Vec<Field { key: &str, value: &str, line: usize }>`.

Validation layer (separate from parsing) checks required/optional field constraints
and emits `Diagnostic`s with accurate line numbers from the parse output.

---

## CLI interface

```
# Auto-discover from cwd
agentlint

# Explicit paths
agentlint .claude/agents/debugger.md AGENTS.md

# JSON output for CI tooling
agentlint --format json

# Suppress non-zero exit (audit mode)
agentlint --exit-zero
```

Exit codes:

- `0` — all files valid (or `--exit-zero` set)
- `1` — one or more validation errors
- `2` — internal error (I/O failure, etc.)

---

## Tech decisions

| Decision            | Choice                         | Rationale                                                                  |
| ------------------- | ------------------------------ | -------------------------------------------------------------------------- |
| Parser combinator   | `nom`                          | Precise byte-level control, accurate line tracking, zero alloc in hot path |
| Arg parsing         | `clap` (derive)                | Standard, low boilerplate                                                  |
| JSON output         | `serde_json`                   | Already present for opencode.json parsing                                  |
| Workspace structure | lib crates + thin bin          | Core and per-agent crates reusable as library deps                         |
| Error type          | `Vec<Diagnostic>` not `Result` | Validators accumulate all errors, not fail-fast                            |

---

## Out of scope (v1)

- Validation of `CLAUDE.md` / `GEMINI.md` content quality (just presence)
- Schema validation of `opencode.json` beyond JSON well-formedness
- `model` field value validation (known model IDs change frequently)
- `tools`/`disallowedTools` value validation for Claude Code agents/skills/commands
- Hook script correctness (shellcheck, nu syntax) — existence + shebang only
- Auto-fix / `--fix` mode
- VS Code / LSP integration
- Pi agent-specific file paths under `~/.pi/agent/` (project-root only for v1)
