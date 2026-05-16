# `.agentlint.toml` Full Config Support

**Date**: 2026-05-16
**Status**: approved

## Goal

Replace the hand-rolled TOML line-scanner in `main.rs` with a proper config system
that supports per-rule severity overrides and per-path ignore entries, in addition to
the existing `difficulty` setting. The config drives a passthrough model: consuming
agents and CI pipelines see exactly the severity declared in config, making agentlint
composable in agentic pipelines.

## Config file format

```toml
[agentlint]
difficulty = "hard"   # easy | hard | painful  (default: hard)

[rules]
# Per-rule severity override: "error" | "warning" | "off"
"claude/settings/broad-read"          = "off"
"claude/hooks/naive-str-contains"     = "error"

[[ignore]]
path  = ".claude/settings.local.json"   # suffix match against diagnostic path
rules = ["claude/settings/broad-read"]  # empty = suppress all rules for this path
```

`.agentlint.toml` is searched in the process working directory. Not required — all
fields have defaults.

## Architecture

### Crates affected

| Crate            | Changes                                                          |
| ---------------- | ---------------------------------------------------------------- |
| `agentlint-core` | New `config` feature: `config.rs` module, `ConfigError`,         |
|                  | `load_config()`, expanded `RunConfig`, updated filtering in      |
|                  | `run_on`                                                         |
| `agentlint`      | Enable `agentlint-core/config`; replace hand-rolled scanner with |
| (bin)            | `load_config()`; merge CLI `--difficulty` on top                 |

No new crates. No changes to validator crates.

### New types (`agentlint-core`, `config` feature)

```rust
pub enum RuleOverride {
    Error,
    Warning,
    Off,
}

pub struct IgnoreEntry {
    pub path: String,         // suffix matched against diagnostic path
    pub rules: Vec<String>,   // empty = suppress all rules for this path
}

// RunConfig gains two fields:
pub struct RunConfig {
    pub difficulty:     Difficulty,
    pub rule_overrides: HashMap<String, RuleOverride>,
    pub ignores:        Vec<IgnoreEntry>,
}

pub struct ConfigError(String);   // thin newtype, no external error crate
```

### Config loading

```rust
/// Load `.agentlint.toml` from `path`.
/// - Returns `Ok(None)` when the file does not exist.
/// - Returns `Ok(Some(RunConfig))` on success.
/// - Returns `Err(ConfigError)` when the file exists but is malformed.
pub fn load_config(path: &Path) -> Result<Option<RunConfig>, ConfigError>
```

Internal raw structs (not public) mirror the TOML shape and derive
`serde::Deserialize`. `RuleOverride` derives `Deserialize` with renamed variants.

### Filtering logic in `run_on`

Applied in order after all validators run:

1. **Difficulty** — drop `d` where `d.rule != "" && d.difficulty > config.difficulty`
2. **Ignore** — drop `d` where any `IgnoreEntry` matches:
   - `d.path` ends with `entry.path` (OS-normalised suffix)
   - `entry.rules` is empty **or** contains `d.rule`
3. **Override** — look up `d.rule` in `config.rule_overrides`:
   - `Off` → drop
   - `Error` → `d.severity = Severity::Error`
   - `Warning` → `d.severity = Severity::Warning`

Unclassified diagnostics (`rule = ""`) skip steps 2 and 3 — no key to match.

### New dependencies (feature-gated)

```toml
# agentlint-core/Cargo.toml
[features]
config = ["dep:toml", "dep:serde"]

[dependencies]
toml  = { version = "0.8", optional = true }
serde = { version = "1",   features = ["derive"], optional = true }
```

```toml
# Cargo.toml (workspace root / binary)
agentlint-core = { path = "crates/agentlint-core", features = ["config"] }
```

`serde_json` (already present in `agentlint-core`) does not need `derive` — only
the new `toml` deserialization path needs it.

## Test plan

### Unit — filtering logic

All in `crates/agentlint-core/src/lib.rs` `#[cfg(test)]`:

- `difficulty_filter_drops_painful_at_hard`
- `difficulty_filter_passes_painful_at_painful`
- `ignore_filter_suppresses_matching_rule_for_matching_path`
- `ignore_filter_empty_rules_suppresses_all_rules_for_path`
- `ignore_filter_does_not_suppress_different_path`
- `override_off_drops_diagnostic`
- `override_error_promotes_warning`
- `override_warning_demotes_error`
- `unclassified_diagnostic_passes_all_filters`
- `filter_order_difficulty_before_ignore` (painful diagnostic not in ignores list —
  dropped by difficulty, not by ignore)

### Unit — config loading

All in `crates/agentlint-core/src/config.rs` `#[cfg(test)]`:

- `missing_file_returns_none`
- `empty_toml_returns_default_config`
- `difficulty_field_parsed`
- `rules_section_parsed`
- `ignore_section_parsed`
- `ignore_without_rules_field_defaults_to_empty_vec`
- `invalid_toml_returns_err`
- `unknown_difficulty_value_returns_err`

### Property — path suffix matching

In `crates/agentlint-core/src/config.rs` proptest block:

- Any path whose string representation ends with the ignore `path` field is matched
- Any path that does not end with the ignore `path` field is not matched

### Integration — end-to-end

In `crates/agentlint-core/tests/config_integration.rs`:

- Write fixture files + `.agentlint.toml` to `FixtureDir`; run `agentlint_core::run()`
  with loaded config; assert suppressed/promoted diagnostics appear correctly

## Tech decisions

- **`toml` + `serde` feature-gated** — keeps `agentlint-core` dep-free for library
  consumers who build their own config loading
- **Exact suffix match for ignore paths** — avoids glob complexity and "why isn't my
  rule firing?" surprises; trivial to promote to glob later using the existing engine
- **Passthrough severity model** — `"error"/"warning"` overrides change the emitted
  severity directly; downstream agents and CI see exactly what config declares
- **Filtering in `run_on`** not in validators — keeps validators pure and rule-unaware;
  all policy lives in the runner
- **`ConfigError` as newtype** — avoids pulling in `thiserror`/`anyhow`; the one
  display use case (print + exit 2 in main) needs only `Display`

## Out of scope

- Per-path difficulty overrides (use `[[ignore]]` instead)
- Glob patterns in `[[ignore]]` path field
- Remote or URL-based config
- Config validation of rule ID strings (unknown rule IDs in `[rules]` are silently ignored)
- `--config` flag to specify a non-default config path
- Config inheritance / cascading (project + user + global)
