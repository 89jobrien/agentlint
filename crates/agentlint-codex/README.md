# agentlint-codex

Validates Codex agent files.

## Files validated

| Pattern     | Description              |
| ----------- | ------------------------ |
| `AGENTS.md` | Codex agent instructions |

## Checks

- File is non-empty
- Has at least one markdown heading
- Minimum content threshold (5 non-empty lines, 100 non-whitespace chars)
