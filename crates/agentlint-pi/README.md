# agentlint-pi

Validates Pi agent files.

## Files validated

| Pattern     | Description        |
| ----------- | ------------------ |
| `AGENTS.md` | Agent instructions |
| `SYSTEM.md` | System prompt      |

## Checks

- File is non-empty
- Has at least one markdown heading
- Minimum content threshold (5 non-empty lines, 100 non-whitespace chars)
