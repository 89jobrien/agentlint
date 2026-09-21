# Consolidate per-agent crates into declarative plugins + core behavioral module

## Goal

Eliminate 7 per-agent Rust crates. TOML plugins own all structural rules.
Behavioral logic (not expressible in TOML) moves to
`agentlint-core::behavioral`. Cross-file checks become a `BatchCheck` trait.
The result: dropping a `.toml` in `plugins/` is the only step to add a new
agent harness validator. Rust code is only needed for behavioral rules.

## Architecture

### New modules in agentlint-core

```text
src/behavioral/
  mod.rs                 -- registry: HashMap<&str, BehavioralFn>
  claude/
    mod.rs
    hooks.rs             -- sleep, sshpass, infinite-loop, naive-str-match, execute-bit
    settings.rs          -- permissions, hooks structure, env scanning, warn patterns
    mcp.rs               -- secret detection, transport, duplicate server names
    agents.rs            -- name-collision (per-field beyond TOML)
    skills.rs            -- name-format (kebab-case, dir match)
  cursor/
    mod.rs
    rules.rs             -- never-fires, invalid-always-apply, invalid-globs
  looprs/
    mod.rs
    commands.rs          -- YAML nested field validation
    hooks.rs             -- trigger matching
```

### Registry

```rust
type BehavioralFn = fn(&Path, &str, &[Field]) -> Vec<Diagnostic>;

pub fn registry() -> HashMap<&'static str, BehavioralFn> { ... }
```

Populated once at startup from a static list. Each entry is a pure function.

### TOML dispatch

New check type in declarative engine:

```toml
[[validator.rules]]
check = "custom"
custom_fn = "claude-hooks-sleep"
```

`eval.rs` looks up `custom_fn` in the registry and calls it.

### BatchCheck trait

```rust
pub trait BatchCheck: Send + Sync {
    fn patterns(&self) -> &[&str];
    fn check(&self, files: &[(&Path, &str)]) -> Vec<Diagnostic>;
}
```

Runner collects matched files per BatchCheck, calls after main loop.
`validate_batch` removed from Validator trait.

Initial impl: `DuplicateAgentNames` in `behavioral::claude::agents`.

### build.rs auto-discovery (already done)

`crates/agentlint-plugins/build.rs` scans `plugins/*.toml` and generates
the `BUILTIN_PLUGINS` array. No manual registration needed.

## Crates deleted

| Crate              | Reason                                       |
| ------------------ | -------------------------------------------- |
| agentlint-codex    | 100% covered by TOML                         |
| agentlint-gemini   | 100% covered by TOML                         |
| agentlint-pi       | 100% covered by TOML                         |
| agentlint-opencode | 100% covered by TOML                         |
| agentlint-cursor   | Structural in TOML, behavioral moves to core |
| agentlint-claude   | Structural in TOML, behavioral moves to core |
| agentlint-looprs   | Structural in TOML, behavioral moves to core |

## Crates kept

| Crate                 | Role                                                               |
| --------------------- | ------------------------------------------------------------------ |
| agentlint-core        | Validator trait, declarative engine, behavioral module, BatchCheck |
| agentlint-frontmatter | nom-based parser (shared by TOML engine + behavioral)              |
| agentlint-docs        | DocsValidator, schema inference (separate concern)                 |
| agentlint-plugins     | build.rs auto-discovery, embedded + external plugin loading        |

## Migration phases

### Phase 1: Infrastructure

- Add `behavioral/` module tree to core with registry
- Add `BatchCheck` trait
- Add `check = "custom"` + `custom_fn` to declarative types and eval
- Add `agentlint-frontmatter` as a dependency of core (needed by behavioral)

### Phase 2: Move behavioral logic

- Copy behavioral functions from each crate into core
- Register in registry
- Move duplicate-agent-names to `BatchCheck` impl
- Remove `validate_batch` from `Validator` trait
- Move tests alongside the functions

### Phase 3: Update TOML plugins

- Add `custom_fn` rules to claude/cursor/looprs TOML files
- Verify no structural duplication remains

### Phase 4: Delete crates

- Remove 7 crate directories
- Remove from workspace Cargo.toml (members + deps)
- Remove from binary Cargo.toml (dependencies + dev-dependencies)

### Phase 5: Update wiring

- `src/main.rs`: remove per-agent imports, use only plugins + BatchCheck
- `tests/integration.rs`: use plugin validators
- `tests/conformance.rs`: same
- `xtask/src/detect_changes.rs`: update crate name references

## Out of scope

- Extending the TOML engine with regex-based checks (option C from brainstorm)
- Docs crate consolidation (separate concern, schema inference is complex)
- Plugin versioning or external plugin API stability

## Verification

1. `cargo check --workspace`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo nextest run --workspace` -- all tests pass
4. Run `agentlint` against a repo with Claude, Cursor, and Codex files
5. Confirm diagnostic output matches pre-refactor output (no duplicates,
   no missing rules)
