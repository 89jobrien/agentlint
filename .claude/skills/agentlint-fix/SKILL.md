---
name: agentlint-fix
description: >
  Run agentlint to validate AI agent harness files and auto-fix violations.
  Use when the user says "lint agent files", "fix agentlint errors",
  "validate my claude config", "check my cursor rules", "run agentlint",
  or any request to find and fix problems in agent configuration files
  (.claude/, .cursor/, AGENTS.md, CLAUDE.md, .mcp.json, etc). Also use
  when the user asks to "clean up" or "fix" their agent harness setup.
---

# agentlint-fix

Validate AI agent harness files with agentlint, then fix reported violations.

## Step 1: Run agentlint

```bash
agentlint --format json 2>/dev/null
```

If agentlint is not installed, install it first:

```bash
cargo install --path /Users/joe/dev/agentlint
```

Parse the JSON output. Each diagnostic has: `path`, `line`, `col`, `message`,
`severity` (error/warning), and `rule_id`.

If there are zero diagnostics, report success and stop.

## Step 2: Triage by difficulty

Group diagnostics by fix difficulty. Fix in this order:

### Easy fixes (apply without asking)

| Rule pattern                           | Fix                                                                |
| -------------------------------------- | ------------------------------------------------------------------ |
| `missing-frontmatter`                  | Prepend `---\nname: <inferred>\ndescription: <placeholder>\n---\n` |
| `missing required field 'name'`        | Add `name: <filename-stem>` to frontmatter                         |
| `missing required field 'description'` | Add `description: TODO` to frontmatter                             |
| `invalid JSON`                         | Parse error — read file, fix syntax                                |
| `missing-shebang`                      | Prepend `#!/usr/bin/env nu` (or bash if not .nu)                   |
| `unclosed frontmatter fence`           | Add closing `---` after last frontmatter field                     |
| `non-empty`                            | Flag to user — file is empty, needs content                        |
| `must have a top-level 'mcpServers'`   | Wrap content in `{"mcpServers": {...}}`                            |

### Moderate fixes (show plan, apply on confirmation)

| Rule pattern                  | Fix                                                     |
| ----------------------------- | ------------------------------------------------------- |
| `unknown top-level key`       | Remove or rename the key; show which keys are valid     |
| `description is too short`    | Expand the description — read the file body for context |
| `exceeds N lines`             | Suggest splitting into smaller files                    |
| `no markdown headings`        | Add a `# Title` heading based on filename               |
| `name collides with built-in` | Suggest a rename; show the collision                    |

### Hard fixes (present options, let user decide)

| Rule pattern                    | Fix                                              |
| ------------------------------- | ------------------------------------------------ |
| MCP transport issues            | Show valid transport config examples             |
| Secret detection (`env` values) | Replace hardcoded values with `$ENV_VAR` refs    |
| `op://` URI warnings            | Explain the issue and suggest `op read` wrapping |
| Execute permission missing      | Run `chmod +x <path>`                            |

## Step 3: Apply fixes

1. Read each file before modifying it
2. Apply easy fixes directly
3. For moderate/hard fixes, show the proposed change and wait for confirmation
4. After all fixes, re-run agentlint to verify:

```bash
agentlint --format json 2>/dev/null
```

Report remaining issues if any.

## Frontmatter format reference

Agent, skill, and command files use YAML frontmatter:

```markdown
---
name: my-agent
description: A helpful description of what this agent does
---

Body content here.
```

- The `---` fences must be on their own lines
- `name` and `description` are always required
- Skill names must be kebab-case and match the parent directory name
- Command names derive from the filename path

## MCP config format reference

`.mcp.json` uses stdio or HTTP transport:

```json
{
  "mcpServers": {
    "my-server": {
      "command": "node",
      "args": ["server.js"],
      "env": {
        "API_KEY": "$API_KEY"
      }
    }
  }
}
```

- Every server needs `command` (stdio) or `url` (HTTP/SSE)
- Never hardcode secrets in `env` — use `$ENV_VAR` references
- `op://` URIs don't resolve in Claude's shell; use `op read` instead

## Settings keys reference

Valid top-level keys in `.claude/settings.json`:

`permissions`, `env`, `hooks`, `mcpServers`, `model`, `apiKeyHelper`,
`includeCoAuthoredBy`, `enabledMcpjsonServers`, `cleanupPeriodDays`,
`effortLevel`, `enabledPlugins`, `extraKnownMarketplaces`, `fastMode`,
`skipDangerousModePermissionPrompt`, `statusLine`, `verbose`, `attribution`
