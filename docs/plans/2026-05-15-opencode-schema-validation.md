# agentlint-opencode — Schema Validation Design

**Date**: 2026-05-15
**Status**: approved
**Author**: Joseph O'Brien

---

## Goal

Replace the current `OpenCodeJsonValidator` (which only checks JSON well-formedness) with
a typed schema validator that catches unknown fields and type mismatches. Unknown fields
emit a **warning** (version skew is common); type mismatches emit a **error** (always a
bug).

---

## Architecture

**Crate affected**: `agentlint-opencode` only — no new crates.

**New types**:

- `OpenCodeError` — custom error enum, one variant per failure mode
- `OpenCodeConfig` — serde `Deserialize` struct mirroring the opencode.json top-level shape

**Data flow**:

```
src (raw &str)
  │
  ▼
serde_json::from_str::<Value>          → InvalidJson error (line, col from serde_json::Error)
  │
  ▼
walk top-level object keys             → UnknownField warnings (accumulates ALL unknowns)
  │
  ▼
for each known key, type-check value   → TypeMismatch errors
  │
  ▼
Vec<OpenCodeError>  →  map to Vec<Diagnostic>
```

Two-phase approach (Value walk then type-check) is required because
`#[serde(deny_unknown_fields)]` stops at the first unknown and loses the rest.

---

## Custom error type

```rust
/// A validation finding for `opencode.json`.
#[derive(Debug, PartialEq)]
pub enum OpenCodeError {
    /// JSON is not syntactically valid.
    InvalidJson {
        line: usize,
        col: usize,
        message: String,
    },
    /// A top-level key is not in the known schema.
    /// Severity: Warning — opencode evolves quickly; version skew is expected.
    UnknownField {
        field: String,
    },
    /// A known field has the wrong JSON type.
    /// Severity: Error — always a user mistake.
    TypeMismatch {
        field: &'static str,
        expected: &'static str,
        got: &'static str,
    },
    /// A known field has a value outside the allowed set.
    /// Severity: Error.
    InvalidValue {
        field: &'static str,
        allowed: &'static [&'static str],
        got: String,
    },
}

impl OpenCodeError {
    pub fn severity(&self) -> agentlint_core::Severity {
        match self {
            Self::UnknownField { .. } => Severity::Warning,
            _ => Severity::Error,
        }
    }

    pub fn message(&self) -> String { ... }
}
```

`OpenCodeJsonValidator::validate` collects `Vec<OpenCodeError>` internally, then maps each
to a `Diagnostic` using `OpenCodeError::severity()` and `::message()`.

---

## Known top-level keys

Sourced from `packages/opencode/src/config/config.ts` in the `sst/opencode` repo
(`Schema.Struct` definition, commit pinned in code comment):

```
$schema, shell, logLevel, server, command, skills, reference, watcher,
snapshot, plugin, share, autoshare, autoupdate, disabled_providers,
enabled_providers, model, small_model, default_agent, username, mode,
agent, provider, mcp, formatter, lsp, instructions, layout, permission,
tools, attachment, enterprise, tool_output, compaction, experimental
```

Stored as `const KNOWN_KEYS: &[&str]` in `lib.rs`.

---

## Type-checked fields (top-level only for v1)

| Field                | Expected type                      | Variant on failure |
| -------------------- | ---------------------------------- | ------------------ |
| `model`              | string                             | `TypeMismatch`     |
| `small_model`        | string                             | `TypeMismatch`     |
| `shell`              | string                             | `TypeMismatch`     |
| `username`           | string                             | `TypeMismatch`     |
| `default_agent`      | string                             | `TypeMismatch`     |
| `$schema`            | string                             | `TypeMismatch`     |
| `snapshot`           | boolean                            | `TypeMismatch`     |
| `autoshare`          | boolean                            | `TypeMismatch`     |
| `instructions`       | array                              | `TypeMismatch`     |
| `disabled_providers` | array                              | `TypeMismatch`     |
| `enabled_providers`  | array                              | `TypeMismatch`     |
| `logLevel`           | `"DEBUG"\|"INFO"\|"WARN"\|"ERROR"` | `InvalidValue`     |
| `share`              | `"manual"\|"auto"\|"disabled"`     | `InvalidValue`     |
| `autoupdate`         | boolean or `"notify"`              | `TypeMismatch`     |

Nested fields (`agent.build.model`, `mcp.<name>.command`, etc.) are **out of scope for
v1** — structural presence is enough.

---

## Diagnostics mapping

```rust
fn to_diagnostic(path: &Path, e: &OpenCodeError) -> Diagnostic {
    Diagnostic {
        path: path.to_path_buf(),
        line: 1,
        col: 1,
        severity: e.severity(),
        message: e.message(),
    }
}
```

`InvalidJson` uses the line/col from `serde_json::Error`; all other variants report
`(1, 1)` because top-level key positions are not tracked in the Value walk.

---

## Tests

TDD — write failing tests first, then implement.

- `valid_minimal_is_clean` — `{}` emits nothing
- `valid_full_known_keys_is_clean` — all known keys with correct types
- `unknown_key_is_warning` — `{"theme": "dark"}` → 1 warning, 0 errors
- `multiple_unknown_keys_all_reported` — accumulates, does not stop at first
- `model_wrong_type_is_error` — `{"model": 42}` → 1 error
- `log_level_invalid_value_is_error` — `{"logLevel": "TRACE"}` → 1 error
- `share_invalid_value_is_error` — `{"share": "yes"}` → 1 error
- `snapshot_wrong_type_is_error` — `{"snapshot": "yes"}` → 1 error
- `invalid_json_is_error` — `{bad}` → 1 error with message containing "invalid JSON"
- `deprecated_autoshare_is_warning` — `{"autoshare": true}` → clean (no warning for
  deprecated but valid fields)
- `unknown_and_type_error_both_reported` — accumulates across phases

---

## Out of scope (v1)

- Nested field validation (`agent`, `mcp`, `provider`, `formatter`, `lsp`, etc.)
- `autoupdate: "notify"` vs boolean distinction (accept any string or bool)
- `model` format validation (`provider/model` pattern)
- Deprecated field warnings (`autoshare`, `mode`, `layout`)
- JSON Schema generation or `$schema` URL resolution

---

## Dependencies

No new workspace dependencies. `serde_json` is already present in `agentlint-opencode`.
