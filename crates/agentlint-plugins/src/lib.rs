//! Embedded declarative plugin definitions for agentlint.
//!
//! Each plugin TOML is compiled into the binary via `include_str!` and parsed
//! at startup. External plugins can also be loaded from a directory at runtime.

use agentlint_core::Validator;
use agentlint_core::declarative::{load_plugin_validators, validators_from_str};
use std::path::Path;

const CLAUDE_TOML: &str = include_str!("../../../plugins/agentlint.claude.toml");
const CURSOR_TOML: &str = include_str!("../../../plugins/agentlint.cursor.toml");
const CODEX_TOML: &str = include_str!("../../../plugins/agentlint.codex.toml");
const GEMINI_TOML: &str = include_str!("../../../plugins/agentlint.gemini.toml");
const OPENCODE_TOML: &str = include_str!("../../../plugins/agentlint.opencode.toml");
const PI_TOML: &str = include_str!("../../../plugins/agentlint.pi.toml");
const LOOPRS_TOML: &str = include_str!("../../../plugins/agentlint.looprs.toml");

static BUILTIN_PLUGINS: &[(&str, &str)] = &[
    ("claude", CLAUDE_TOML),
    ("cursor", CURSOR_TOML),
    ("codex", CODEX_TOML),
    ("gemini", GEMINI_TOML),
    ("opencode", OPENCODE_TOML),
    ("pi", PI_TOML),
    ("looprs", LOOPRS_TOML),
];

/// Parse and return validators from all embedded plugin definitions.
///
/// Panics if any builtin plugin TOML is malformed (this is a compile-time
/// invariant — the files are baked in).
pub fn all_validators() -> Vec<Box<dyn Validator>> {
    let mut out = Vec::new();
    for (name, src) in BUILTIN_PLUGINS {
        match validators_from_str(src) {
            Ok(vs) => out.extend(vs),
            Err(e) => panic!("builtin plugin '{name}' is invalid: {e}"),
        }
    }
    out
}

/// Load external plugin validators from all `.toml` files in `dir`.
///
/// Returns an empty vec if the directory doesn't exist. Errors from
/// individual files are collected and returned; valid plugins are still
/// loaded.
pub fn load_external(dir: &Path) -> (Vec<Box<dyn Validator>>, Vec<String>) {
    let mut validators = Vec::new();
    let mut errors = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return (validators, errors),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            match load_plugin_validators(&path) {
                Ok(vs) => validators.extend(vs),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
    }

    (validators, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_plugins_parse() {
        let vs = all_validators();
        assert!(
            vs.len() >= 10,
            "expected at least 10 validators, got {}",
            vs.len()
        );
    }

    #[test]
    fn each_plugin_has_patterns() {
        for (name, src) in BUILTIN_PLUGINS {
            let vs = validators_from_str(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            for v in &vs {
                assert!(
                    !v.patterns().is_empty(),
                    "{name}: validator has no patterns"
                );
            }
        }
    }

    #[test]
    fn load_external_missing_dir_is_empty() {
        let (vs, errs) = load_external(Path::new("/nonexistent"));
        assert!(vs.is_empty());
        assert!(errs.is_empty());
    }
}
