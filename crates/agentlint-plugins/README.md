# agentlint-plugins

Embedded declarative TOML plugin definitions for agentlint.

## How it works

Each plugin TOML file under `plugins/` is compiled into the binary via
`include_str!` and parsed at startup. The `all_validators()` function
returns boxed `Validator` impls for all builtin plugins.

External plugins can also be loaded from a directory at runtime via
`load_external(path)`.

## Builtin plugins

| Plugin   | TOML file                         |
| -------- | --------------------------------- |
| claude   | `plugins/agentlint.claude.toml`   |
| cursor   | `plugins/agentlint.cursor.toml`   |
| codex    | `plugins/agentlint.codex.toml`    |
| gemini   | `plugins/agentlint.gemini.toml`   |
| opencode | `plugins/agentlint.opencode.toml` |
| pi       | `plugins/agentlint.pi.toml`       |
| looprs   | `plugins/agentlint.looprs.toml`   |

## Adding a new plugin

1. Create `plugins/agentlint.<name>.toml`
2. Add `const <NAME>_TOML` and entry to `BUILTIN_PLUGINS` in `src/lib.rs`
3. Run tests to verify parsing
