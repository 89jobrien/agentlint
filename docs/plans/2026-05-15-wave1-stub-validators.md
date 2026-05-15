# Plan: Wave 1 — Stub Validator Implementations

## Goal

Implement the five stub validators (cursor, codex, opencode, gemini, pi) with their
v1 validation rules so `agentlint` produces real diagnostics for all supported agent
harness file formats.

## Architecture

- Crates affected: `agentlint-cursor`, `agentlint-codex`, `agentlint-opencode`,
  `agentlint-gemini`, `agentlint-pi`
- New traits/types: none — each crate already has a `*Validator` struct implementing
  `agentlint_core::Validator`
- Data flow: `validate(path, src)` → `Vec<Diagnostic>` per crate
- Shared pattern: "non-empty content" check reused across codex/gemini/pi/opencode;
  Cursor has optional frontmatter parsing via the nom parser already in
  `agentlint-claude::frontmatter` (exposed as pub or duplicated — see Task 1)

## Tech Stack

- Rust edition 2024
- `nom = "7"` — already in `agentlint-claude`; add to `agentlint-cursor`
- `serde_json = "1"` — already in workspace; add to `agentlint-opencode`
- `agentlint-core` test-utils feature for assertion helpers in dev-dependencies

## Tasks

### Task 1: agentlint-cursor — optional frontmatter validation

**Crate**: `agentlint-cursor`
**File(s)**: `crates/agentlint-cursor/Cargo.toml`,
`crates/agentlint-cursor/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-cursor`

Cursor `.mdc`/`.md` files may have optional YAML frontmatter. When frontmatter is
present (file starts with `---\n`), validate it is well-formed. No required fields —
only structural errors are emitted.

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
            "---\ndescription: lint all files\nglobs: \"**/*.rs\"\nalwaysApply: true\n---\n# body\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn unclosed_fence_is_error() {
        let diags = CursorValidator.validate(
            Path::new(".cursor/rules/foo.mdc"),
            "---\ndescription: lint all files\n# body (no closing fence)\n",
        );
        assert!(
            diags.iter().any(|d| d.message.contains("unclosed frontmatter")),
            "expected unclosed-frontmatter error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-cursor -- tests`
Expected: FAIL (validate returns `vec![]` always)

2. Add dependencies to `crates/agentlint-cursor/Cargo.toml`:

```toml
[dependencies]
agentlint-core = { path = "../agentlint-core" }
nom             = "7"

[dev-dependencies]
agentlint-core = { path = "../agentlint-core", features = ["test-utils"] }
```

3. Implement `crates/agentlint-cursor/src/lib.rs`:

```rust
use agentlint_core::{Diagnostic, Validator};
use nom::{
    IResult,
    bytes::complete::{tag, take_until},
    sequence::delimited,
};
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
        if !src.starts_with("---\n") {
            return vec![];
        }
        // Frontmatter present — verify it closes.
        if parse_frontmatter(src).is_err() {
            return vec![Diagnostic::error(
                path,
                1,
                1,
                "unclosed frontmatter: missing closing `---`",
            )];
        }
        vec![]
    }
}

/// Returns the frontmatter content between the two `---` fences, or an error.
fn parse_frontmatter(src: &str) -> IResult<&str, &str> {
    delimited(tag("---\n"), take_until("\n---"), tag("\n---"))(src)
}
```

4. Verify:

```
cargo nextest run -p agentlint-cursor    → all green
cargo clippy -p agentlint-cursor -- -D warnings  → zero warnings
```

5. Run: `git branch --show-current`
   Verify output is `main` (or your feature branch). Commit:
   `git commit -m "feat(cursor): optional frontmatter fence validation"`

---

### Task 2: agentlint-codex — non-empty AGENTS.md

**Crate**: `agentlint-codex`
**File(s)**: `crates/agentlint-codex/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-codex`

Codex requires `AGENTS.md` to be non-empty (not blank/whitespace-only). Hard error
if the file is empty or whitespace-only.

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

Gemini requires `GEMINI.md` to be non-empty.

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

Pi requires `AGENTS.md` and `SYSTEM.md` to be non-empty. Both share the same check.

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agents_non_empty_is_clean() {
        let diags = PiValidator.validate(
            Path::new("AGENTS.md"),
            "# Agent\n\nInstructions.\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn system_non_empty_is_clean() {
        let diags = PiValidator.validate(
            Path::new("SYSTEM.md"),
            "You are a helpful assistant.\n",
        );
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
            return vec![Diagnostic::error(
                path,
                1,
                1,
                format!("{name} is empty"),
            )];
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

### Task 5: agentlint-opencode — AGENTS.md non-empty + opencode.json valid JSON

**Crate**: `agentlint-opencode`
**File(s)**: `crates/agentlint-opencode/Cargo.toml`,
`crates/agentlint-opencode/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-opencode`

OpenCode validates two files: `AGENTS.md` (non-empty) and `opencode.json`
(well-formed JSON). The dispatch is by filename since the validator claims both patterns.

1. Write failing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agents_non_empty_is_clean() {
        let diags = OpenCodeValidator.validate(
            Path::new("AGENTS.md"),
            "# OpenCode Instructions\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn agents_empty_is_error() {
        let diags = OpenCodeValidator.validate(Path::new("AGENTS.md"), "");
        assert!(
            diags.iter().any(|d| d.message.contains("empty")),
            "expected empty-file error, got: {diags:?}",
        );
    }

    #[test]
    fn opencode_json_valid_is_clean() {
        let diags = OpenCodeValidator.validate(
            Path::new("opencode.json"),
            r#"{"model": "claude-sonnet-4-6"}"#,
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn opencode_json_empty_object_is_clean() {
        let diags = OpenCodeValidator.validate(Path::new("opencode.json"), "{}");
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn opencode_json_invalid_is_error() {
        let diags = OpenCodeValidator.validate(
            Path::new("opencode.json"),
            "{bad json",
        );
        assert!(
            diags.iter().any(|d| d.message.contains("invalid JSON")),
            "expected invalid-JSON error, got: {diags:?}",
        );
    }
}
```

Run: `cargo nextest run -p agentlint-opencode -- tests`
Expected: FAIL

2. Add dependencies to `crates/agentlint-opencode/Cargo.toml`:

```toml
[package]
name = "agentlint-opencode"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
agentlint-core = { path = "../agentlint-core" }
serde_json      = "1"

[dev-dependencies]
agentlint-core = { path = "../agentlint-core", features = ["test-utils"] }
```

3. Implement `crates/agentlint-opencode/src/lib.rs`:

```rust
use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct OpenCodeValidator;

impl Validator for OpenCodeValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "opencode.json"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        match path.file_name().and_then(|n| n.to_str()) {
            Some("AGENTS.md") => validate_agents_md(path, src),
            Some("opencode.json") => validate_opencode_json(path, src),
            _ => vec![],
        }
    }
}

fn validate_agents_md(path: &Path, src: &str) -> Vec<Diagnostic> {
    if src.trim().is_empty() {
        return vec![Diagnostic::error(path, 1, 1, "AGENTS.md is empty")];
    }
    vec![]
}

fn validate_opencode_json(path: &Path, src: &str) -> Vec<Diagnostic> {
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
```

4. Verify:

```
cargo nextest run -p agentlint-opencode    → all green
cargo clippy -p agentlint-opencode -- -D warnings  → zero warnings
```

5. Run: `git branch --show-current`
   Commit: `git commit -m "feat(opencode): AGENTS.md non-empty + opencode.json JSON validation"`

---

### Task 6: workspace integration — nextest all green

**Crate**: workspace
**File(s)**: none
**Run**: `cargo nextest run --workspace`

After all five tasks above are committed, verify the full workspace is green and
clippy-clean.

1. Run:

```
cargo nextest run --workspace
```

Expected: all tests pass, zero failures.

2. Run:

```
cargo clippy --workspace -- -D warnings
```

Expected: zero warnings.

3. Push:

```
git push
```

---

## Quality Rules

- No placeholders: every code block is copy-paste ready.
- Exact paths: all file paths match the actual workspace layout.
- TDD for every task: failing test confirmed before implementation.
- Each task ends with a commit.
- Tasks 2–4 are independent and can run in parallel (no shared files).
- Tasks 1 and 5 require Cargo.toml edits before the impl step.
