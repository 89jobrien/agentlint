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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agents_non_empty_is_clean() {
        let diags =
            AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), "# OpenCode Instructions\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn agents_empty_is_error() {
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), "");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("empty"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn opencode_json_valid_is_clean() {
        let diags = OpenCodeJsonValidator.validate(
            Path::new("opencode.json"),
            r#"{"model": "claude-sonnet-4-6"}"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn opencode_json_empty_object_is_clean() {
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), "{}");
        assert!(diags.is_empty());
    }

    #[test]
    fn opencode_json_invalid_is_error() {
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), "{bad json");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("invalid JSON"),
            "message: {}",
            diags[0].message
        );
    }
}
