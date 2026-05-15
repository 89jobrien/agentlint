# Plan: Wave 1 — Stub Validator Implementations

## Goal

Implement the five stub validators (cursor, codex, opencode, gemini, pi) with their
v1 validation rules so `agentlint` produces real diagnostics for all supported agent
harness file formats.

## Architecture

Layer stack (lowest to highest):

```
agentlint (bin)               — composition root
agentlint-{claude,cursor,...} — adapters, one struct per file-type concern
agentlint-frontmatter         — shared YAML frontmatter parser utility
agentlint-core                — primitives only: Diagnostic, Validator, run_on, run
```

Rules:

- No adapter depends on another adapter crate.
- `agentlint-core` has no parser logic — only the port trait and runner primitives.
- `agentlint-frontmatter` has no validator logic — only parsing and field extraction.
- Each `*Validator` struct owns exactly one file-type concern.
- `validate(path, src)` receives pre-loaded content — no filesystem access inside
  any validator.

Crates affected: `agentlint-frontmatter` (new), `agentlint-core` (Cargo.toml: drop
nom), `agentlint-claude` (use frontmatter crate), `agentlint-cursor`,
`agentlint-codex`, `agentlint-opencode`, `agentlint-gemini`, `agentlint-pi`,
`agentlint` (bin, Cargo.toml: add frontmatter if needed — likely not).

## Tech Stack

- Rust edition 2024
- `nom = "7"` — moves to `agentlint-frontmatter`
- `serde_json = "1"` — stays in `agentlint-core` (format_json) and `agentlint-opencode`
- `agentlint-core` test-utils feature for assertion helpers in dev-dependencies

---

## Tasks

### Task 0: Create agentlint-frontmatter crate

**Crate**: `agentlint-frontmatter` (new), `agentlint-core`, `agentlint-claude`
**File(s)**:

- `crates/agentlint-frontmatter/Cargo.toml` (new)
- `crates/agentlint-frontmatter/src/lib.rs` (new)
- `Cargo.toml` (workspace: add member)
- `crates/agentlint-core/Cargo.toml` (remove nom)
- `crates/agentlint-claude/Cargo.toml` (replace nom with agentlint-frontmatter)
- `crates/agentlint-claude/src/frontmatter.rs` (re-export)
  **Run**: `cargo nextest run --workspace`

1. Create `crates/agentlint-frontmatter/Cargo.toml`:

```toml
[package]
name    = "agentlint-frontmatter"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
publish.workspace = true

[dependencies]
agentlint-core = { path = "../agentlint-core" }
nom             = "7"
```

2. Create `crates/agentlint-frontmatter/src/lib.rs` — copy the entire body of
   `crates/agentlint-claude/src/frontmatter.rs` verbatim (the module already has full
   tests; they travel with the move). The `use agentlint_core::Diagnostic;` import
   at the top is correct as-is.

3. Add the new member to the root `Cargo.toml` `[workspace]` members list:

```toml
members = [
    "crates/agentlint-core",
    "crates/agentlint-frontmatter",   # add this line
    "crates/agentlint-claude",
    ...
]
```

4. Remove `nom` from `crates/agentlint-core/Cargo.toml` (agentlint-core never used
   it — it was in claude only). Verify `crates/agentlint-core/Cargo.toml` has:

```toml
[dependencies]
serde_json = "1"
walkdir    = "2"
tempfile   = { version = "3", optional = true }
```

5. Update `crates/agentlint-claude/Cargo.toml` — replace `nom` with
   `agentlint-frontmatter`:

```toml
[dependencies]
agentlint-core        = { path = "../agentlint-core" }
agentlint-frontmatter = { path = "../agentlint-frontmatter" }
serde_json            = "1"

[dev-dependencies]
agentlint-core = { path = "../agentlint-core", features = ["test-utils"] }
tempfile        = "3"
```

6. Replace `crates/agentlint-claude/src/frontmatter.rs` with a re-export so all
   existing call sites (`use super::frontmatter::...`) continue to resolve:

```rust
//! Frontmatter parser — re-exported from agentlint-frontmatter.
pub use agentlint_frontmatter::{Field, ParseError, check_required, parse};
```

7. Verify:

```
cargo nextest run --workspace    → all green (frontmatter tests now in agentlint-frontmatter)
cargo clippy --workspace -- -D warnings  → zero warnings
```

8. Run: `git branch --show-current`
   Commit: `git commit -m "refactor: extract agentlint-frontmatter crate, decouple parser from claude adapter"`

---

### Task 1: agentlint-cursor — optional frontmatter validation

**Crate**: `agentlint-cursor`
**File(s)**: `crates/agentlint-cursor/Cargo.toml`,
`crates/agentlint-cursor/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-cursor`

Requires Task 0 complete.

Cursor `.mdc`/`.md` files have optional YAML frontmatter. When present (file starts
with `---\n`), validate it is well-formed using `agentlint_frontmatter::parse`.
No required fields — only the fence structure is checked.

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn no_frontmatter_is_clean() {
        let diags = CursorValidator.validate(
            Path::new(".cursor/rules/foo.mdc"),
            "# My rule\n\nDo something.\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn well_formed_frontmatter_is_clean() {
        let diags = CursorValidator.validate(
            Path::new(".cursor/rules/foo.mdc"),
            "---\ndescription: lint all files\nglobs: \"**/*.rs\"\n---\n# body\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn unclosed_fence_is_error() {
        let diags = CursorValidator.validate(
            Path::new(".cursor/rules/foo.mdc"),
            "---\ndescription: lint all files\n# no closing fence\n",
        );
        assert!(
            diags.iter().any(|d| d.message.contains("unclosed")),
            "expected unclosed-fence error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-cursor -- tests`
Expected: FAIL

2. Update `crates/agentlint-cursor/Cargo.toml`:

```toml
[package]
name    = "agentlint-cursor"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
agentlint-core        = { path = "../agentlint-core" }
agentlint-frontmatter = { path = "../agentlint-frontmatter" }

[dev-dependencies]
agentlint-core = { path = "../agentlint-core", features = ["test-utils"] }
```

3. Implement `crates/agentlint-cursor/src/lib.rs`:

```rust
use agentlint_core::{Diagnostic, Validator};
use agentlint_frontmatter::{ParseError, parse};
use std::path::Path;

pub struct CursorValidator;

impl Validator for CursorValidator {
    fn patterns(&self) -> &[&str] {
        &[
            ".cursor/rules/**/*.mdc",
            ".cursor/rules/**/*.md",
            ".cursorrules",
        ]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        // Frontmatter is optional — only validate when the opening fence is present.
        if !src.starts_with("---\n") && !src.starts_with("---\r\n") {
            return vec![];
        }
        match parse(src) {
            Ok(_) => vec![],
            Err(ParseError::UnclosedFence) => vec![Diagnostic::error(
                path,
                1,
                1,
                "unclosed frontmatter fence: missing closing '---'",
            )],
            Err(ParseError::NoFence) => vec![], // unreachable given the starts_with guard
        }
    }
}
```

4. Verify:

```
cargo nextest run -p agentlint-cursor    → all green
cargo clippy -p agentlint-cursor -- -D warnings  → zero warnings
```

5. Run: `git branch --show-current`
   Commit: `git commit -m "feat(cursor): optional frontmatter fence validation"`

---

### Task 2: agentlint-codex — non-empty AGENTS.md

**Crate**: `agentlint-codex`
**File(s)**: `crates/agentlint-codex/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-codex`

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn non_empty_is_clean() {
        let diags = CodexValidator.validate(
            Path::new("AGENTS.md"),
            "# Agent Instructions\n\nDo things.\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn empty_file_is_error() {
        let diags = CodexValidator.validate(Path::new("AGENTS.md"), "");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }

    #[test]
    fn whitespace_only_is_error() {
        let diags = CodexValidator.validate(Path::new("AGENTS.md"), "   \n\n  \t\n");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-codex -- tests`
Expected: FAIL

2. Implement `crates/agentlint-codex/src/lib.rs`:

```rust
use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct CodexValidator;

impl Validator for CodexValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![Diagnostic::error(path, 1, 1, "AGENTS.md is empty")];
        }
        vec![]
    }
}
```

3. Verify:

```
cargo nextest run -p agentlint-codex    → all green
cargo clippy -p agentlint-codex -- -D warnings  → zero warnings
```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(codex): non-empty AGENTS.md validation"`

---

### Task 3: agentlint-gemini — non-empty GEMINI.md

**Crate**: `agentlint-gemini`
**File(s)**: `crates/agentlint-gemini/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-gemini`

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn non_empty_is_clean() {
        let diags = GeminiValidator.validate(
            Path::new("GEMINI.md"),
            "# Gemini Instructions\n\nDo things.\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn empty_file_is_error() {
        let diags = GeminiValidator.validate(Path::new("GEMINI.md"), "");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }

    #[test]
    fn whitespace_only_is_error() {
        let diags = GeminiValidator.validate(Path::new("GEMINI.md"), "\n\n  \n");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-gemini -- tests`
Expected: FAIL

2. Implement `crates/agentlint-gemini/src/lib.rs`:

```rust
use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct GeminiValidator;

impl Validator for GeminiValidator {
    fn patterns(&self) -> &[&str] {
        &["GEMINI.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![Diagnostic::error(path, 1, 1, "GEMINI.md is empty")];
        }
        vec![]
    }
}
```

3. Verify:

```
cargo nextest run -p agentlint-gemini    → all green
cargo clippy -p agentlint-gemini -- -D warnings  → zero warnings
```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(gemini): non-empty GEMINI.md validation"`

---

### Task 4: agentlint-pi — non-empty AGENTS.md and SYSTEM.md

**Crate**: `agentlint-pi`
**File(s)**: `crates/agentlint-pi/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-pi`

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agents_non_empty_is_clean() {
        let diags = PiValidator.validate(Path::new("AGENTS.md"), "# Agent\n\nInstructions.\n");
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn system_non_empty_is_clean() {
        let diags = PiValidator.validate(Path::new("SYSTEM.md"), "You are helpful.\n");
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn agents_empty_is_error() {
        let diags = PiValidator.validate(Path::new("AGENTS.md"), "");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }

    #[test]
    fn system_empty_is_error() {
        let diags = PiValidator.validate(Path::new("SYSTEM.md"), "  \n");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-pi -- tests`
Expected: FAIL

2. Implement `crates/agentlint-pi/src/lib.rs`:

```rust
use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct PiValidator;

impl Validator for PiValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "SYSTEM.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            return vec![Diagnostic::error(path, 1, 1, format!("{name} is empty"))];
        }
        vec![]
    }
}
```

3. Verify:

```
cargo nextest run -p agentlint-pi    → all green
cargo clippy -p agentlint-pi -- -D warnings  → zero warnings
```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(pi): non-empty AGENTS.md and SYSTEM.md validation"`

---

### Task 5: agentlint-opencode — two validators, one concern each

**Crate**: `agentlint-opencode`, `agentlint` (bin)
**File(s)**: `crates/agentlint-opencode/Cargo.toml`,
`crates/agentlint-opencode/src/lib.rs`,
`src/main.rs`
**Run**: `cargo nextest run -p agentlint-opencode && cargo check -p agentlint`

OpenCode owns two file types with different validation rules. Each gets its own
struct — one concern per adapter. No internal filename dispatch.

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agents_non_empty_is_clean() {
        let diags = AgentsMarkdownValidator.validate(
            Path::new("AGENTS.md"),
            "# OpenCode Instructions\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn agents_empty_is_error() {
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), "");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }

    #[test]
    fn opencode_json_valid_is_clean() {
        let diags = OpenCodeJsonValidator.validate(
            Path::new("opencode.json"),
            r#"{"model": "claude-sonnet-4-6"}"#,
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn opencode_json_empty_object_is_clean() {
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), "{}");
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn opencode_json_invalid_is_error() {
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), "{bad json");
        assert!(
            diags.iter().any(|d| d.message.contains("invalid JSON")),
            "expected invalid-JSON error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-opencode -- tests`
Expected: FAIL

2. `crates/agentlint-opencode/Cargo.toml` — verify (already correct, no change):

```toml
[dependencies]
agentlint-core = { path = "../agentlint-core" }
serde_json      = "1"

[dev-dependencies]
agentlint-core = { path = "../agentlint-core", features = ["test-utils"] }
```

3. Replace `crates/agentlint-opencode/src/lib.rs` entirely:

```rust
use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

/// Validates that OpenCode's `AGENTS.md` is non-empty.
pub struct AgentsMarkdownValidator;

impl Validator for AgentsMarkdownValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![Diagnostic::error(path, 1, 1, "AGENTS.md is empty")];
        }
        vec![]
    }
}

/// Validates that `opencode.json` is well-formed JSON.
pub struct OpenCodeJsonValidator;

impl Validator for OpenCodeJsonValidator {
    fn patterns(&self) -> &[&str] {
        &["opencode.json"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if let Err(e) = serde_json::from_str::<serde_json::Value>(src) {
            return vec![Diagnostic::error(
                path,
                e.line(),
                e.column(),
                format!("invalid JSON: {e}"),
            )];
        }
        vec![]
    }
}
```

4. Update `src/main.rs` validators list — replace the single `OpenCodeValidator`
   line with the two new structs:

```rust
// remove:
Box::new(agentlint_opencode::OpenCodeValidator),

// add:
Box::new(agentlint_opencode::AgentsMarkdownValidator),
Box::new(agentlint_opencode::OpenCodeJsonValidator),
```

5. Verify:

```
cargo nextest run -p agentlint-opencode    → all green
cargo check -p agentlint                  → no errors
cargo clippy --workspace -- -D warnings   → zero warnings
```

6. Run: `git branch --show-current`
   Commit: `git commit -m "feat(opencode): split into AgentsMarkdownValidator + OpenCodeJsonValidator"`

---

### Task 6: workspace integration

**Crate**: workspace
**Run**: `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

After all prior tasks are committed, verify the full workspace is green:

```
cargo nextest run --workspace
cargo clippy --workspace -- -D warnings
git push
```

---

## Dependency order

```
Task 0 (frontmatter crate) ──► Task 1 (cursor)
                            ──► Task 5 (opencode) — independent of Task 1

Tasks 2, 3, 4 — fully independent, run in parallel

Task 6 — after all others
```
