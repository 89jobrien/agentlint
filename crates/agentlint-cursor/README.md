# agentlint-cursor

Validates Cursor rule files.

## Files validated

| Pattern                  | Description         |
| ------------------------ | ------------------- |
| `.cursor/rules/**/*.mdc` | MDC rule files      |
| `.cursor/rules/**/*.md`  | Markdown rule files |
| `.cursorrules`           | Legacy rules file   |

## Checks

- Frontmatter is well-formed if present (unclosed fence is an error)
- `description` is a non-empty string if present
- `globs` is a string or list of strings if present
- `alwaysApply` is a boolean if present

Files without a frontmatter fence are silently skipped.

## Dependencies

Uses `agentlint-frontmatter` for YAML frontmatter parsing.
