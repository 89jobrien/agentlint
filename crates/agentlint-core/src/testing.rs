//! Test infrastructure for agentlint — fixture builders and assertion helpers.
//!
//! Enabled via the `test-utils` Cargo feature so downstream crates can use
//! these in their own `[dev-dependencies]` without `cfg(test)` restrictions.
//!
//! # Usage
//!
//! ```rust,ignore
//! use agentlint_core::testing::{FixtureDir, assert_error_contains, assert_no_errors};
//!
//! let dir = FixtureDir::new();
//! let path = dir.write(".claude/agents/foo.md", "---\nname: foo\ndescription: bar\n---\n");
//! let diags = MyValidator.validate(&path, &std::fs::read_to_string(&path).unwrap());
//! assert_no_errors(&diags);
//! ```

use std::path::{Path, PathBuf};
use tempfile::TempDir;

use crate::{Diagnostic, Severity};

// ---------------------------------------------------------------------------
// FixtureDir
// ---------------------------------------------------------------------------

/// Temporary directory fixture for writing harness files under a fresh root.
///
/// The underlying [`TempDir`] is cleaned up when this struct is dropped.
pub struct FixtureDir {
    /// The root temporary directory (kept alive for Drop).
    pub dir: TempDir,
}

impl FixtureDir {
    /// Create a new empty fixture directory.
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().expect("TempDir::new"),
        }
    }

    /// Write `content` to `rel_path` inside the fixture root, creating
    /// intermediate directories as needed. Returns the absolute path.
    pub fn write(&self, rel_path: &str, content: &str) -> PathBuf {
        let path = self.dir.path().join(rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create_dir_all");
        }
        std::fs::write(&path, content).expect("write fixture");
        path
    }

    /// The absolute path to the fixture root.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

impl Default for FixtureDir {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Assert that `diags` contains at least one error whose message contains
/// `msg`. Panics with a readable diff if not found.
pub fn assert_error_contains(diags: &[Diagnostic], msg: &str) {
    assert!(
        diags
            .iter()
            .any(|d| matches!(d.severity, Severity::Error) && d.message.contains(msg)),
        "expected error containing {:?}\ngot diagnostics:\n{}",
        msg,
        fmt_diags(diags),
    );
}

/// Assert that `diags` contains at least one error at `line` whose message
/// contains `msg`.
pub fn assert_error_at(diags: &[Diagnostic], line: usize, msg: &str) {
    assert!(
        diags.iter().any(|d| {
            matches!(d.severity, Severity::Error) && d.line == line && d.message.contains(msg)
        }),
        "expected error at line {line} containing {:?}\ngot diagnostics:\n{}",
        msg,
        fmt_diags(diags),
    );
}

/// Assert that `diags` contains no errors. Warnings are allowed.
pub fn assert_no_errors(diags: &[Diagnostic]) {
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "expected no errors\ngot:\n{}",
        fmt_diags(&errors.into_iter().cloned().collect::<Vec<_>>()),
    );
}

/// Assert that `diags` is completely empty (no errors or warnings).
pub fn assert_clean(diags: &[Diagnostic]) {
    assert!(
        diags.is_empty(),
        "expected no diagnostics\ngot:\n{}",
        fmt_diags(diags),
    );
}

/// Verify that a [`Validator`] implementation satisfies the trait contract:
///
/// 1. `patterns()` returns a non-empty slice
/// 2. `validate(dummy_path, "")` does not panic
/// 3. `validate_batch(&[])` returns an empty vec
/// 4. `validate(dummy_path, <random UTF-8>)` does not panic (basic fuzz)
pub fn assert_validator_contract(v: &dyn crate::Validator) {
    // 1. patterns() is non-empty
    let patterns = v.patterns();
    assert!(
        !patterns.is_empty(),
        "Validator::patterns() must return at least one pattern"
    );

    // 2. validate on empty string does not panic
    let dummy = std::path::PathBuf::from(patterns[0].replace("**/*", "test").replace('*', "test"));
    let _ = v.validate(&dummy, "");

    // 3. validate_batch on empty slice returns empty vec
    let batch = v.validate_batch(&[]);
    assert!(
        batch.is_empty(),
        "validate_batch(&[]) must return empty vec, got {} diagnostics",
        batch.len()
    );

    // 4. validate on various UTF-8 inputs does not panic
    let fuzz_inputs = [
        "hello world",
        "---\n---\n",
        "---\nname: test\ndescription: test\n---\nbody",
        "{ \"key\": \"value\" }",
        "{}\n",
        "# Heading\n\n## Sub\n\nContent line one.\nContent line two.\n\
         Content line three.\nContent line four.\nContent line five.",
        "\x00\x01\x02",
        "\u{FEFF}BOM content",
        &"a".repeat(10_000),
        "---\n\x00: bad\n---\n",
    ];
    for input in &fuzz_inputs {
        let _ = v.validate(&dummy, input);
    }
}

/// Assert that a [`Validator`] produces at least one error when given an
/// empty file at `filename`.
pub fn assert_validator_rejects_empty(v: &dyn crate::Validator, filename: &str) {
    let path = std::path::PathBuf::from(filename);
    let diags = v.validate(&path, "");
    assert!(
        diags.iter().any(|d| matches!(d.severity, Severity::Error)),
        "expected at least one error for empty file {:?}\ngot diagnostics:\n{}",
        filename,
        fmt_diags(&diags),
    );
}

fn fmt_diags(diags: &[Diagnostic]) -> String {
    if diags.is_empty() {
        return "  (none)".to_string();
    }
    diags
        .iter()
        .map(|d| format!("  {}", d.gnu_format()))
        .collect::<Vec<_>>()
        .join("\n")
}
