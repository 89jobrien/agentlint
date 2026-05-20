# agentlint-claude

Validates Claude Code harness files.

## Files validated

| Pattern                    | Kind                                                   |
| -------------------------- | ------------------------------------------------------ |
| `.claude/agents/**/*.md`   | Agent definitions (frontmatter: name, description)     |
| `.claude/skills/**/*.md`   | Skill definitions (frontmatter: name, description)     |
| `.claude/commands/**/*.md` | Command definitions (frontmatter: name, description)   |
| `.claude/hooks/**`         | Hook scripts (shebang, execute bit)                    |
| `.claude/settings.json`    | Settings (known keys, permission shapes)               |
| `.mcp.json`                | MCP server config (transport, env secrets, duplicates) |
| `CLAUDE.md`                | Meta file (heading present, line count)                |

## Architecture

Single `ClaudeValidator` classifies paths by `ClaudeFileKind` and
delegates to sub-module logic. A private `frontmatter` module wraps
the shared `agentlint-frontmatter` parser.

Sub-modules: `agents`, `skills`, `commands`, `hooks`, `settings`, `mcp`,
`meta`.

Cross-file check: duplicate agent names across all `.claude/agents/`
files (via `validate_batch`).
