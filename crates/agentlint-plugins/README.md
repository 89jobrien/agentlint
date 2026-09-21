# agentlint-plugins

Build-time embedding and runtime loading for agentlint's declarative TOML validators. This crate is
an adapter around `agentlint_core::declarative`; it contains no independent rule evaluator.

## Workspace role

The workspace no longer needs one Rust crate per supported harness. Structural rules live in
`plugins/*.toml`, complex checks live in `agentlint_core::behavioral`, and this crate turns TOML
into `Box<dyn Validator>` values for the CLI.

```rust
let mut validators = agentlint_plugins::all_validators();
let (external, errors) = agentlint_plugins::load_external(std::path::Path::new("plugins"));
validators.extend(external);
```

## Build-time embedding

`build.rs` scans the workspace's `plugins/` directory for every `.toml` file, sorts entries by file
name, and generates `OUT_DIR/builtin_plugins.rs`. Each generated entry uses `include_str!` with the
canonical source path. Cargo reruns the build script when the directory or an individual plugin
changes.

There is no handwritten built-in list. Adding or removing a top-level TOML file changes the next
build automatically. `all_validators()` parses every embedded source at startup and panics if one is
invalid, treating valid built-ins as a build/release invariant.

The current workspace plugin set is:

| Plugin         | Main file patterns and formats                                             |
| -------------- | -------------------------------------------------------------------------- |
| `claude`       | Claude agents, skills, commands, hooks, settings, MCP, and `CLAUDE.md`.    |
| `cursor`       | Optional-frontmatter Cursor rules and `.cursorrules`.                      |
| `codex`        | Markdown quality checks for `AGENTS.md`.                                   |
| `gemini`       | Markdown quality checks for `GEMINI.md`.                                   |
| `opencode`     | `AGENTS.md` and JSON checks for `opencode.json`.                           |
| `pi`           | Markdown quality checks for `AGENTS.md` and `SYSTEM.md`.                   |
| `looprs`       | YAML, frontmatter, Markdown, and JSON under `.looprs` plus `*.agent.json`. |
| `maestro-docs` | Markdown checks for Maestro feature-oriented documentation directories.    |

Because discovery is automatic, this table describes the checked-out tree rather than defining the
authoritative set.

## Runtime plugins

`load_external(dir)` reads each direct child whose extension is exactly `.toml`. A missing or
unreadable directory yields empty validators and no errors. Valid files are retained even when other
files fail; parse/read errors are returned as strings prefixed with their paths. Directory iteration
order is filesystem-defined.

The CLI calls `load_external(Path::new("plugins"))` after loading embedded validators. When running
from a source checkout, the same workspace plugin files are therefore available both as embedded
built-ins and as runtime files; callers that use the library can choose a separate external plugin
directory or omit runtime loading.

**Warning:** Running the CLI with the repository as its working directory can load identical
validators from the embedded plugin set and the external `plugins/` directory. The loader does not
deduplicate them, so matching rules may emit duplicate diagnostics.

## Plugin contract

A plugin contains one metadata table, optional constants, and one or more validators:

```toml
[plugin]
name = "example"
prefix = "example"

[plugin.constants]
allowed_kinds = ["guide", "reference"]

[[validator]]
id = "docs"
patterns = ["docs/**/*.md"]
format = "frontmatter"
frontmatter_optional = true
skip_filenames = ["README.md"]

[[validator.rules]]
id = "example/docs/unknown-kind"
check = "field-one-of"
field = "kind"
values = "$allowed_kinds"
severity = "warning"
difficulty = "hard"
message = "unknown documentation kind"
```

`format` is `yaml`, `json`, `frontmatter`, or `markdown`. Rules run in order and default to error
severity and easy difficulty. Available checks and their parameters are defined by
`agentlint_core::declarative::{Check, RuleDef}`; see the `agentlint-core` README for the complete
list. Rule IDs conventionally use `<plugin>/<category>/<slug>` because configuration, ignores, and
output expose them directly.

Constants are TOML arrays referenced as `$constant_name`. Unknown or non-array constants resolve to
an empty value list. `skip_filenames` compares final path components case-insensitively.
`frontmatter_optional` skips files with no fence but still permits an unclosed-fence rule to report
a malformed fence.

Structural TOML and native behavioral validators may claim the same path. This is intentional: the
core runner executes every match, allowing TOML to cover schema rules while Rust checks secrets,
cross-file state, command semantics, or other behavior.

## Adding or changing a plugin

1. Add or edit `plugins/agentlint.<name>.toml`.
2. Use existing check names and provide stable rule IDs, messages, severities, and difficulties.
3. Add native behavior under `agentlint_core::behavioral` only when declarative checks are
   insufficient, then assemble that validator in the CLI.
4. Run the plugin tests and the workspace conformance/integration suites.

No `src/lib.rs` registration edit is required for a new top-level TOML file.

```console
cargo nextest run -p agentlint-plugins
cargo nextest run --test conformance
cargo nextest run --test integration
cargo clippy -p agentlint-plugins -- -D warnings
cargo fmt --all --check
```

The crate tests require all embedded files to parse, require every generated validator to advertise
at least one pattern, and verify that a missing external directory is harmless.
