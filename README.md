# agentlint

Linter for AI coding agent harness files. Validates agents, skills, commands, hooks, and
settings across the major coding agent platforms.

```
$ agentlint
.claude/agents/debugger.md:1:1: error: missing required field 'name'
.claude/commands/deploy.md:3:1: error: missing required field 'description'
.claude/settings.json:1:1: error: unknown top-level key 'theme'
3 errors
```

## Supported agents

| Agent       | Files validated                                                                                                            |
| ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | `.claude/agents/**/*.md`, `.claude/skills/**/*.md`, `.claude/commands/**/*.md`, `.claude/hooks/*`, `.claude/settings.json` |
| Cursor      | `.cursor/rules/**/*.mdc`, `.cursor/rules/**/*.md`, `.cursorrules`                                                          |
| Codex       | `AGENTS.md`                                                                                                                |
| OpenCode    | `AGENTS.md`, `opencode.json`                                                                                               |
| Gemini      | `GEMINI.md`                                                                                                                |
| Pi          | `AGENTS.md`, `SYSTEM.md`                                                                                                   |

## Install

```
cargo install agentlint
```

## Usage

```
# Validate all agent harness files in the current directory
agentlint

# Validate specific files or directories
agentlint .claude/agents/ AGENTS.md

# JSON output for CI tooling
agentlint --format json

# Audit mode — report issues but always exit 0
agentlint --exit-zero
```

### Exit codes

| Code | Meaning                                  |
| ---- | ---------------------------------------- |
| `0`  | All files valid                          |
| `1`  | One or more validation errors            |
| `2`  | Internal error (I/O, permission failure) |

## What is validated

### Claude Code

**Agents, skills, commands** (`.claude/agents/`, `.claude/skills/`, `.claude/commands/`):

- `name` field present and non-empty
- `description` field present and non-empty
- Frontmatter fence is well-formed (`---` … `---`)

**Hooks** (`.claude/hooks/*`):

- First line is a shebang (`#!`)
- File has execute permission

**Settings** (`.claude/settings.json`, `.claude/settings.local.json`):

- Valid JSON
- No unknown top-level keys
- `permissions.allow` / `permissions.deny` are arrays of strings if present
- `hooks` entries have a `command` string field if present

### Cursor

**Rules** (`.cursor/rules/`, `.cursorrules`):

- Frontmatter is well-formed if present (unclosed fence is an error)
- `description` is a non-empty string if present
- `globs` is a string or list of strings if present
- `alwaysApply` is a boolean if present

### Codex, OpenCode, Gemini, Pi

- File is non-empty and not whitespace-only
- `opencode.json` is valid JSON

## CI integration

Add to `.github/workflows/`:

```yaml
- name: agentlint
  run: agentlint
```

Or with cargo:

```yaml
- uses: actions/checkout@v4
- uses: dtolnay/rust-toolchain@stable
- run: cargo install agentlint
- run: agentlint
```

## Development

```bash
# First-time setup (configures git hooks, installs cargo tools)
./scripts/dev-setup.sh

cargo check --workspace
cargo nextest run --workspace
cargo clippy --workspace -- -D warnings
```

See [`docs/plans/2026-05-15-agentlint.md`](docs/plans/2026-05-15-agentlint.md) for the
full design document and [`docs/ROADMAP.agentlint.md`](docs/ROADMAP.agentlint.md) for
shipped milestones and planned work.

## License

MIT OR Apache-2.0
