# agentlint-core

Core primitives for agentlint: the `Diagnostic` type, `Validator` trait,
file discovery, output formatters, runner, and declarative plugin engine.

## Key types

| Type           | Description                                                                  |
| -------------- | ---------------------------------------------------------------------------- |
| `Diagnostic`   | Single lint finding: path, line, col, severity, message, rule ID, difficulty |
| `Severity`     | `Error` or `Warning`                                                         |
| `Difficulty`   | Gating tier: `Easy` < `Normal` < `Hard` < `Painful`                          |
| `Validator`    | Trait every per-platform crate implements                                    |
| `RunConfig`    | Difficulty filter, rule overrides, path ignores                              |
| `RunResult`    | Collected diagnostics + file count                                           |
| `OutputFormat` | `Gnu`, `Json`, or `Pretty`                                                   |

## Validator trait

```rust
pub trait Validator: Send + Sync {
    fn patterns(&self) -> &[&str];
    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic>;
    fn validate_batch(&self, files: &[(PathBuf, String)]) -> Vec<Diagnostic> { vec![] }
}
```

Per-platform crates implement `Validator`. The runner matches files to
validators by glob pattern and collects diagnostics.

## Runner

Two entry points:

- `run_on(files, validators, config)` -- pure domain runner, no I/O
- `run(roots, validators, config)` -- walks directories, reads files,
  delegates to `run_on`

Filtering pipeline: difficulty gate -> path ignores -> rule overrides.

## Modules

| Module        | Feature flag  | Description                                                |
| ------------- | ------------- | ---------------------------------------------------------- |
| `declarative` | `declarative` | TOML plugin engine (see `src/declarative/README.md`)       |
| `config`      | `config`      | TOML config file loading (`.agentlint.toml`)               |
| `testing`     | `test-utils`  | `FixtureDir`, assertion helpers for downstream crate tests |

## Output formatters

- `format_gnu()` -- `path:line:col: severity[rule]: message`
- `format_json()` -- JSON array of diagnostic objects
- `format_pretty()` -- ANSI-colored, grouped by file
