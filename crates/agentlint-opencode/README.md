# agentlint-opencode

Validates OpenCode agent and config files.

## Files validated

| Pattern         | Description            |
| --------------- | ---------------------- |
| `AGENTS.md`     | Agent instructions     |
| `opencode.json` | OpenCode configuration |

## Checks

**AGENTS.md**: non-empty, has heading, minimum content threshold.

**opencode.json**: valid JSON, top-level object.
