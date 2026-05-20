---
name: agentlint-plugin
description: >
  Author declarative TOML plugins for agentlint — define file patterns,
  validation rules, and constants without writing Rust. Use when the user
  asks to "write an agentlint plugin", "add a validator", "create lint
  rules for X", "add agentlint support for a new platform", or wants to
  extend agentlint with custom validation rules. Also use when discussing
  the agentlint plugin format or asking how to add new checks.
---

# agentlint-plugin

Write declarative TOML plugins that agentlint compiles into validators.
No Rust code required for structural/schema checks.

## Plugin file structure

A plugin is a single `.toml` file with three sections:

```toml
[plugin]
name = "my-platform"
prefix = "my-platform"

[plugin.constants]
# Reusable value lists referenced by rules via "$name"
valid_statuses = ["draft", "active", "archived"]

[[validator]]
id = "config-files"
patterns = [".my-platform/config/*.yaml"]
format = "yaml"           # yaml | json | frontmatter | markdown

[[validator.rules]]
id = "my-platform/config/missing-name"
check = "field-required"
field = "name"
severity = "error"
difficulty = "easy"
message = "missing required field 'name'"
```

## Where plugins live

- **Builtin**: `plugins/agentlint.<name>.toml` — compiled into the binary
  via `include_str!` in `crates/agentlint-plugins/src/lib.rs`
- **External**: any `.toml` in a directory passed to `load_external()`

For a new builtin plugin:

1. Create `plugins/agentlint.<name>.toml`
2. Add `const <NAME>_TOML: &str = include_str!(...)` to `lib.rs`
3. Add entry to `BUILTIN_PLUGINS` array
4. Run `cargo nextest run --workspace` to verify

## Format types

| Format        | Parses as                                     | Use for                                 |
| ------------- | --------------------------------------------- | --------------------------------------- |
| `frontmatter` | YAML between `---` fences, then markdown body | Agent/skill/command files with metadata |
| `yaml`        | Full YAML document                            | Config files (.yaml/.yml)               |
| `json`        | JSON object                                   | Settings, config files                  |
| `markdown`    | Raw text with line-based checks               | CLAUDE.md, AGENTS.md, hook scripts      |

## Available checks

Every rule needs a `check` field. Here are all supported checks:

### Content checks (work with any format)

| Check                 | Fields                   | What it does                                      |
| --------------------- | ------------------------ | ------------------------------------------------- |
| `non-empty`           | —                        | File is not empty/whitespace-only                 |
| `valid-parse`         | —                        | File parses as its declared format                |
| `has-shebang`         | —                        | First line starts with `#!`                       |
| `has-heading`         | —                        | File contains at least one markdown heading       |
| `has-command-heading` | —                        | File has a heading that looks like a command name |
| `max-lines`           | `max`                    | File does not exceed N lines                      |
| `min-content`         | `min_lines`, `min_chars` | File has minimum content                          |

### Frontmatter checks (format = "frontmatter")

| Check                 | Fields            | What it does                                     |
| --------------------- | ----------------- | ------------------------------------------------ |
| `has-frontmatter`     | —                 | File starts with `---` fence                     |
| `frontmatter-closed`  | —                 | Opening `---` has a matching close               |
| `field-required`      | `field`           | Named field exists and is non-empty              |
| `field-min-length`    | `field`, `min`    | Field value is at least N chars                  |
| `field-max-length`    | `field`, `max`    | Field value is at most N chars                   |
| `field-one-of`        | `field`, `values` | Field value is in the allowed set                |
| `field-not-one-of-ci` | `field`, `values` | Field value is NOT in the set (case-insensitive) |

### JSON/YAML object checks (format = "json" or "yaml")

| Check             | Fields   | What it does                              |
| ----------------- | -------- | ----------------------------------------- |
| `is-object`       | —        | Top-level value is an object/mapping      |
| `known-keys`      | `values` | All top-level keys are in the allowed set |
| `field-exists`    | `field`  | Named key exists in the object            |
| `array-non-empty` | `field`  | Named field is a non-empty array          |

## Rule fields reference

```toml
[[validator.rules]]
id = "prefix/category/rule-name"   # unique rule ID (prefix/ matches plugin prefix)
check = "field-required"           # one of the Check enum variants
field = "name"                     # target field (for field-* checks)
values = ["a", "b"]               # inline values, OR
values = "$constant_name"          # reference to [plugin.constants]
min = 10                           # minimum (for length/line checks)
max = 500                          # maximum (for length/line checks)
min_lines = 3                      # minimum non-empty lines (min-content)
min_chars = 50                     # minimum non-whitespace chars (min-content)
severity = "error"                 # "error" (default) or "warning"
difficulty = "easy"                # "easy" (default), "normal", "hard", "painful"
message = "human-readable message" # shown in diagnostic output
```

## Constants and references

Define reusable lists in `[plugin.constants]` and reference with `$name`:

```toml
[plugin.constants]
valid_types = ["api", "cli", "web"]

[[validator.rules]]
id = "my/type-check"
check = "field-one-of"
field = "type"
values = "$valid_types"
message = "type must be one of: api, cli, web"
```

## Validator options

```toml
[[validator]]
id = "my-files"
patterns = [".config/**/*.yaml"]   # glob patterns to match
format = "yaml"
skip_filenames = ["_template.yaml"] # skip specific filenames
frontmatter_optional = true         # don't error on missing frontmatter
```

When `frontmatter_optional = true`, files without a `---` fence are silently
skipped instead of producing parse errors. Useful for formats where frontmatter
is an optional enhancement (like Cursor rules).

## Severity and difficulty

- **severity**: `error` fails the lint run (exit 1), `warning` does not
- **difficulty**: hints for auto-fix tooling
  - `easy` — mechanical fix, safe to auto-apply
  - `normal` — straightforward but needs context
  - `hard` — requires understanding of the broader config
  - `painful` — subjective or architectural decision

## Worked example: adding a new platform

Say you want to lint `.windsurf/rules/*.md` files that need frontmatter with
`name` and `trigger` fields:

```toml
[plugin]
name = "windsurf"
prefix = "windsurf"

[plugin.constants]
valid_triggers = ["always", "manual", "file-match"]

[[validator]]
id = "rules"
patterns = [".windsurf/rules/**/*.md"]
format = "frontmatter"

[[validator.rules]]
id = "windsurf/rules/missing-frontmatter"
check = "has-frontmatter"
severity = "error"
message = "rule file must start with frontmatter (---)"

[[validator.rules]]
id = "windsurf/rules/missing-name"
check = "field-required"
field = "name"
severity = "error"
message = "missing required field 'name'"

[[validator.rules]]
id = "windsurf/rules/missing-trigger"
check = "field-required"
field = "trigger"
severity = "error"
message = "missing required field 'trigger'"

[[validator.rules]]
id = "windsurf/rules/invalid-trigger"
check = "field-one-of"
field = "trigger"
values = "$valid_triggers"
severity = "error"
message = "trigger must be one of: always, manual, file-match"
```

## Testing a new plugin

After writing the TOML file:

```bash
# Verify the TOML parses correctly
cargo nextest run --workspace -E 'test(builtin_plugins_parse)'

# Run the full test suite
cargo nextest run --workspace

# Test against real files
cargo run -- .windsurf/
```

If adding as a builtin, also run `cargo clippy --workspace -- -D warnings`.

## Limitations of declarative plugins

Some checks require Rust code and cannot be expressed in TOML:

- Cross-file checks (duplicate name detection across files)
- Pattern matching inside file content (regex, secret detection)
- Conditional logic (if field A exists, then field B is required)
- Custom parse formats beyond yaml/json/frontmatter/markdown
- Permission checks (execute bit)

For these, implement the `Validator` trait in a per-platform Rust crate.
The declarative plugin handles the structural checks; the Rust crate
handles behavioral checks.
