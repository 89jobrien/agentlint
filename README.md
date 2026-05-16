# agentlint

Linter for AI coding agent harness files. Validates agents, skills, commands, hooks, settings,
MCP config, and docs frontmatter across the major coding agent platforms.

```
$ agentlint
.claude/agents/debugger.md:1:1: error: missing required field 'name'
.claude/commands/deploy.md:3:1: error: missing required field 'description'
.claude/settings.json:1:1: error: unknown top-level key 'theme'
.mcp.json:1:1: error: mcpServers.my-server: server entry must have 'command' or 'url' transport
3 errors, 1 warning
```

## Supported platforms

| Platform    | Files validated                                                                                                                                       |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code | `.claude/agents/**/*.md`, `.claude/skills/**/*.md`, `.claude/commands/**/*.md`, `.claude/hooks/**`, `.claude/settings.json`, `.mcp.json`, `CLAUDE.md` |
| Cursor      | `.cursor/rules/**/*.mdc`, `.cursor/rules/**/*.md`, `.cursorrules`                                                                                     |
| Codex       | `AGENTS.md`                                                                                                                                           |
| OpenCode    | `AGENTS.md`, `opencode.json`                                                                                                                          |
| Gemini      | `GEMINI.md`                                                                                                                                           |
| Pi          | `AGENTS.md`, `SYSTEM.md`                                                                                                                              |
| Docs        | `docs/**/*.md` (frontmatter schema)                                                                                                                   |

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
- Duplicate `name:` values across all agent files (cross-file check)

**Hooks** (`.claude/hooks/`):

- First line is a shebang (`#!`)
- File has execute permission

**Settings** (`.claude/settings.json`, `.claude/settings.local.json`):

- Valid JSON
- No unknown top-level keys
- `permissions.allow` / `permissions.deny` are arrays of strings if present
- `hooks` entries have a `command` string field if present

**MCP config** (`.mcp.json`):

- Valid JSON with a top-level `mcpServers` object
- Each server entry has `command` (stdio) or `url` (HTTP/SSE) transport
- `command` is non-empty and not a relative path (relative paths break when Claude Code is
  launched from a different directory)
- `env` values are not hardcoded secrets — must use `$ENV_VAR` or `op://` references
- `op://` URIs in `env` are flagged: they do not resolve in Claude's shell context
- Duplicate server names (last entry silently wins)
- Unconstrained HTTP fetch servers flagged as a security warning
- No unknown fields inside server entries

**CLAUDE.md** (`CLAUDE.md`, `**/CLAUDE.md`):

- File has at least one markdown heading
- File does not exceed 500 lines

### Cursor

**Rules** (`.cursor/rules/`, `.cursorrules`):

- Frontmatter is well-formed if present (unclosed fence is an error)
- `description` is a non-empty string if present
- `globs` is a string or list of strings if present
- `alwaysApply` is a boolean if present

### Codex, OpenCode, Gemini, Pi

- File is non-empty and not whitespace-only
- `opencode.json` is valid JSON

### Docs (`docs/**/*.md`)

Files without a frontmatter fence are skipped. Files with frontmatter are validated against
the project doc schema:

**Required fields:** `title`, `doctype`, `project`, `status`, `created`, `updated`

**Enum values:**

- `status`: `draft`, `active`, `archived`, `superseded`
- `doctype`: `idea`, `spec`, `plan`, `adr`, `roadmap`, `guide`, `reference`, `runbook`,
  `architecture`, `capability-matrix`, `testing`, `development`, `readme`

**Date fields:** `created` and `updated` must be `YYYY-MM-DD`

**`meta` field** — optional; if present, must be a YAML mapping. Two syntaxes accepted:

```yaml
# Inline JSON (single line)
meta: {"spec": "docs/specs/2026-05-15-agentlint.spec.md", "author": "Joe"}

# Indented YAML keys (multiline)
meta:
  spec: docs/specs/2026-05-15-agentlint.spec.md
  author: Joe
```

Plan docs must include a `meta.spec` key linking to the upstream spec.

**Filename conventions** — two layouts are recognised:

- Repo doc: `docs/{doctype}.{project}.md` — `title`, `doctype`, and `project` must match
  the filename components
- Research doc: `docs/{ideas|specs|plans}/{ref}-{topic}.{doctype}.md` — doctype inferred
  from directory if no explicit suffix; plan docs must include a `meta` block with `spec`

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
full design document and [`docs/roadmap.agentlint.md`](docs/roadmap.agentlint.md) for
shipped milestones and planned work.

## License

MIT OR Apache-2.0
