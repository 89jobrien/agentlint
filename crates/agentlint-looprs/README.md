# agentlint-looprs

Validates Looprs agent harness files (commands, hooks, skills).

## Files validated

| Pattern                      | Description               |
| ---------------------------- | ------------------------- |
| `.looprs/commands/*.yaml`    | Command definitions       |
| `.looprs/hooks/*.yaml`       | Hook definitions          |
| `.looprs/skills/**/SKILL.md` | Skill files (frontmatter) |
| `.looprs/rules/**/*.md`      | Rule documents            |
| `.looprs/config.yaml`        | Agent config              |
| `looprs.agent.json`          | Agent manifest            |

## Checks

**Commands/hooks**: valid YAML, `name` and `description` required,
`action.type` must be a known value.

**Hooks**: `trigger` must be a known event from `KNOWN_TRIGGERS`
(e.g. `SessionStart`, `PreToolUse`, `PostToolUse`). Empty `actions`
array flagged.

**Skills**: frontmatter required with `name` field.

**Rules**: non-empty, has heading. `README.md` files are skipped.

**Config**: valid YAML, `name` required.

**agent.json**: valid JSON, `version` field required.
