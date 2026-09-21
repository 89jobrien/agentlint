# agentlint-core

Domain types, runners, built-in behavioral validation, and the declarative plugin engine for
agentlint. The CLI owns argument parsing and validator assembly; this crate owns validation and
diagnostic contracts.

## Workspace role

`agentlint-core` is the lowest-level workspace crate. `agentlint-frontmatter`, `agentlint-docs`,
`agentlint-plugins`, and the `agentlint` binary depend on it. The consolidated architecture keeps
complex Claude, Cursor, and looprs checks in `behavioral`, while structural checks are described by
TOML and executed by `declarative`.

| Feature       | Adds                                                                              |
| ------------- | --------------------------------------------------------------------------------- |
| default       | Diagnostics, runner, formatters, `Validator`, and the minimal frontmatter parser. |
| `config`      | `.agentlint.toml` loading through `config::load_config`.                          |
| `declarative` | TOML schema types, evaluators, and behavioral validators used by the CLI.         |
| `test-utils`  | Temporary fixtures, contract checks, and diagnostic assertions.                   |

The behavioral module currently shares the `declarative` feature gate because custom declarative
checks can call its registry. The CLI enables both `config` and `declarative`.

## Core contracts

### Diagnostics and filtering

`Diagnostic` carries a path, one-based line and column, `Severity`, message, static rule ID, and
`Difficulty`. Use `Diagnostic::error` or `Diagnostic::warning`, then classify the finding:

```rust
use agentlint_core::{Diagnostic, Difficulty};

let diagnostic = Diagnostic::error(".mcp.json", 1, 1, "invalid JSON")
    .with_rule("claude/mcp/invalid-json", Difficulty::Easy);
```

Difficulty is ordered `Easy < Normal < Hard < Painful`; `Hard` is the default run level. Empty rule
IDs are unclassified and bypass difficulty, ignore, and override filters. Classified diagnostics
are processed in this order:

1. discard rules above `RunConfig::difficulty`;
2. apply path-suffix `IgnoreEntry` values;
3. suppress or rewrite severity with `RuleOverride::{Off, Error, Warning}`.

### Validator

```rust
pub trait Validator: Send + Sync {
    fn patterns(&self) -> &[&str];
    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic>;
    fn validate_batch(&self, files: &[(PathBuf, String)]) -> Vec<Diagnostic>;
}
```

`patterns` use agentlint's minimal glob language: literals, `*` within one path segment, and `**`
across directories. Multiple validators may claim the same file, and all matching validators run.
`validate_batch` receives only files claimed by that validator and defaults to no findings; it is
used for corpus-level checks such as duplicate Claude agent names and inferred docs schemas.

`BatchCheck` is also public as a standalone cross-file contract, but the current runner dispatches
batch work through `Validator::validate_batch` rather than accepting `BatchCheck` objects.

## Runners and discovery

`run_on` is the in-memory domain entry point:

```rust
use agentlint_core::{RunConfig, Validator, run_on};
use std::path::PathBuf;

fn lint(files: Vec<(PathBuf, String)>, validators: &[Box<dyn Validator>]) {
    let result = run_on(files, validators, &RunConfig::default());
    assert_eq!(result.files_checked, result.file_counts.values().sum());
}
```

`run` is the filesystem adapter. It walks roots without following links, reads only claimed files,
skips unrecognized and non-UTF-8 files, and converts read failures into diagnostics. Directory
descent is restricted to literal top-level prefixes found in validator patterns. It always skips
`target`, `.git`, `node_modules`, `plugins`, and `.maestro`.

`RunResult` contains filtered diagnostics, the number of distinct matched files, and category counts
for summary output. A file claimed by several validators is counted once but may produce findings
from each validator.

## Declarative plugins

Enable the `declarative` feature to parse plugin TOML:

```rust
use agentlint_core::declarative::validators_from_str;

let source = r#"
[plugin]
name = "example"
prefix = "example"

[[validator]]
id = "instructions"
patterns = ["AGENTS.md"]
format = "markdown"

[[validator.rules]]
id = "example/instructions/empty"
check = "non-empty"
message = "AGENTS.md is empty"
"#;

let validators = validators_from_str(source).expect("valid plugin");
```

`load_plugin_str`, `load_plugin_file`, `validators_from_plugin`,
`load_plugin_validators`, and `validators_from_str` expose the load pipeline. `PluginFile` contains
plugin metadata and one or more `ValidatorDef` values. A validator defines:

- `id`, `patterns`, and one of `yaml`, `json`, `frontmatter`, or `markdown` formats;
- optional `skip_filenames` and `frontmatter_optional` behavior;
- ordered rules with severity, difficulty, message, and check-specific parameters.

Supported checks are `non-empty`, `valid-parse`, `has-frontmatter`, `frontmatter-closed`,
`has-heading`, `has-command-heading`, `has-shebang`, `max-lines`, `min-content`, `field-required`,
`field-exists`, `field-one-of`, `field-not-one-of-ci`, `field-min-length`, `field-max-length`,
`array-non-empty`, `is-object`, `known-keys`, and `custom`. Dotted field paths work for JSON and
YAML. `values` accepts an inline string list or a `$name` reference to a plugin constant array.

Rules execute in declaration order. A failing emptiness, parse, or frontmatter prerequisite stops
evaluation for that file; other failures accumulate. Declarative diagnostics currently report line
1, column 1. `custom` resolves a `custom_fn` through `behavioral::registry`; the current Claude,
Cursor, and looprs register functions add no named entries, so their complex checks are supplied as
native `Validator` implementations instead.

## Behavioral validators

`behavioral::{claude, cursor, looprs}` contains Rust checks that are awkward or unsafe to express as
field predicates. The CLI assembles these native validators alongside declarative plugins:

- `claude::ClaudeValidator` dispatches agents, skills, hooks, settings, MCP, and `CLAUDE.md`, and
  performs duplicate-agent-name batch validation;
- `cursor::CursorValidator` handles behavioral Cursor rule checks;
- looprs exposes validators for commands, hooks, skills, agents, rules, config, and `*.agent.json`.

The public `frontmatter` module is a small parser duplicated inside core to avoid the dependency
cycle that importing `agentlint-frontmatter` would create. Builder-based frontmatter validation
remains in the separate crate.

## Configuration and output

With `config`, `config::load_config(path)` returns `Ok(None)` for a missing file, a `RunConfig` for
valid TOML, or `ConfigError` for read/parse failures. It understands `[agentlint].difficulty`, a
`[rules]` map, and repeated `[[ignore]]` entries.

Output helpers are:

- `format_gnu`: `path:line:col: severity[rule]: message`;
- `format_json`: a pretty JSON array including rule and difficulty;
- `format_pretty`: diagnostics grouped by sorted path, optional ANSI color, and a totals line.

## Testing and development

With `test-utils`, `testing::FixtureDir` creates isolated files and the assertion helpers check
errors, locations, clean results, empty-file rejection, and the general validator contract. The
workspace conformance suite applies that contract to native and embedded declarative validators.

```console
cargo nextest run -p agentlint-core --all-features
cargo nextest run --test conformance
cargo nextest run --test integration
cargo clippy -p agentlint-core --all-features -- -D warnings
cargo fmt --all --check
```
